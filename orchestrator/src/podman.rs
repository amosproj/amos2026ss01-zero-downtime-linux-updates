// Control Podman
// This assumes full ownership of the Podman instance

// This module mainly works using the "podman compose" CLI, and sources the files from an OCI registry
// To remember the version of the respective stack, small marker files are kept in the docker compose cache

use std::{collections::BTreeMap, error::Error, marker::PhantomData, path::Path, sync::Mutex};

use serde::Deserialize;
use tokio::{io::AsyncReadExt, process::Command};

type PodmanErr<R> = Result<R, Box<dyn std::error::Error>>;

// Ensure only a single instance of this ever exists
static INSTANCE: Mutex<Option<Podman<RealPodmanRunner>>> = Mutex::new(Option::Some(Podman {
    runner: RealPodmanRunner,
    version_cache: BTreeMap::new(),
}));

struct Podman<R: PodmanRunner> {
    runner: R,
    version_cache: BTreeMap<String, String>,
}

#[async_trait::async_trait]
trait PodmanRunner {
    async fn run_podman(&mut self, args: &[&str]) -> PodmanErr<Vec<u8>>;
}

impl Podman<RealPodmanRunner> {
    pub fn take() -> PodmanErr<Self> {
        INSTANCE
            .lock()?
            .take()
            .ok_or("Cannot create multiple Podman instances".into())
    }
}

impl<R: PodmanRunner> Podman<R> {
    pub async fn stacks<'a>(&'a mut self) -> PodmanErr<Vec<PodmanStack<'a>>> {
        let cmd_output = self
            .runner
            .run_podman(&["compose", "ls", "--all", "--format", "--json"])
            .await?;

        #[derive(Deserialize)]
        #[serde(rename_all = "PascalCase")]
        struct ResultItem<'a> {
            name: &'a str,
            status: &'a str,
            config_files: &'a str,
        }

        let mut cmd_items: Vec<ResultItem> = serde_json::from_slice(&cmd_output)?;
        cmd_items.sort_by_key(|item| item.name); // Usually already sorted

        let output_futs = cmd_items.iter().map(async |item| {
            let status = match item.status {
                s if s.contains(',') => PodmanStackStatus::Ambiguous,
                s if s.starts_with("running") => PodmanStackStatus::Running,
                _ => PodmanStackStatus::Stopped,
            };

            let version = match self.version_cache.get(item.name) {
                Some(v) => Some(v.to_owned()),
                None => (async || -> PodmanErr<String> {
                    // Try retrieving the stack version by looking in the compose cache folder
                    let mut folders = item.config_files.split(',').map(|s| Path::new(s).parent());
                    let config_folder = match folders.next() {
                        Some(Some(first)) if folders.all(|x| x == Some(first)) => first,
                        _ => return Err("Not sure about config folder".into()),
                    };
                    let path = config_folder.join("version");

                    let file = tokio::fs::File::open(path).await?;
                    let mut output = String::new();
                    file.take(512).read_to_string(&mut output).await?;
                    Ok(output)
                })()
                .await
                .ok(),
            };

            PodmanStack {
                id: item.name.to_owned(),
                status,
                version,
                lifetime: PhantomData,
            }
        });

        let output = futures::future::join_all(output_futs).await;

        // Remove old keys from the version cache
        debug_assert!(output.len() <= self.version_cache.len());
        let mut ids = output.iter().map(|ps| ps.id.as_str()).peekable();
        self.version_cache.retain(|key, _| match ids.peek() {
            Some(s) if s == key => {
                ids.next();
                true
            }
            _ => false,
        });

        Ok(output)
    }

    // Pre-download images to make create() run quicker
    pub async fn pull<'a>(&mut self, source: StackSource<'a>) -> Result<(), Box<dyn Error>> {
        self.runner
            .run_podman(&["compose", "-f", source.0, "pull"])
            .await?;
        Ok(())
    }

    // Create a new stack, start it with start()
    pub async fn create<'a, 'b>(
        &'a mut self,
        id: String,
        source: StackSource<'b>,
        version: String,
    ) -> Result<PodmanStack<'a>, Box<dyn Error>> {
        if self.version_cache.contains_key(&id) {
            return Err("Stack with this ID already exists, consider destroying it first".into());
        }

        self.runner
            .run_podman(&["compose", "-p", &id, "-f", source.0, "create"])
            .await?;

        self.version_cache.insert(id.clone(), version.clone());

        Ok(PodmanStack {
            id,
            status: PodmanStackStatus::Stopped,
            version: Some(version),
            lifetime: PhantomData,
        })
    }
}

#[derive(Clone, Copy)]
struct StackSource<'a>(&'a str);

impl<'a> StackSource<'a> {
    pub fn new(source: &'a str) -> Result<Self, Box<dyn Error>> {
        if !source.starts_with("oci://") {
            Err("Only compose files from OCI registries supported (oci://...)".into())
        } else if source.ends_with("latest") {
            Err("Please use a specific version instead of latest".into())
        } else {
            Ok(Self(source))
        }
    }
}

struct PodmanStack<'a> {
    id: String,
    status: PodmanStackStatus,
    version: Option<String>,
    lifetime: PhantomData<&'a ()>,
}

#[derive(PartialEq, Eq, Debug)]
enum PodmanStackStatus {
    Stopped,
    Ambiguous,
    Running,
}

impl<'a> PodmanStack<'a> {
    pub async fn start(&mut self) {}

    pub async fn stop(&mut self) {}

    pub async fn destroy(self) {}
}

struct RealPodmanRunner;

#[async_trait::async_trait]
impl PodmanRunner for RealPodmanRunner {
    async fn run_podman(&mut self, args: &[&str]) -> PodmanErr<Vec<u8>> {
        match Command::new("/bin/podman").args(args).output().await {
            Err(_) => Err(Box::from("Could not find Podman CLI")),
            Ok(o) if !o.status.success() => Err(match String::from_utf8(o.stderr) {
                Ok(msg) => Box::from(format!("Podman error: {}", msg)),
                _ => Box::from("Podman returned an error"),
            }),
            Ok(o) => Ok(o.stdout),
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;

    struct MockPodmanRunner;

    #[async_trait::async_trait]
    impl super::PodmanRunner for MockPodmanRunner {
        async fn run_podman(&mut self, args: &[&str]) -> super::PodmanErr<Vec<u8>> {
            println!("Running podman {:?}", args);
            match args {
                ["compose", .., "create"] => Ok(Vec::new()),
                _ => Err("Unknown command".into())
            }
        }
    }

    #[tokio::test]
    async fn test_stacks_create() {
        let mut podman = super::Podman {
            runner: MockPodmanRunner,
            version_cache: BTreeMap::new(),
        };

        let stack = podman
            .create(
                "test".to_owned(),
                super::StackSource::new("oci://test:test").unwrap(),
                "1.2.3".to_owned(),
            )
            .await
            .unwrap();

        assert_eq!(stack.id, "test");
        assert_eq!(stack.version, Some("1.2.3".to_owned()));
        assert_eq!(stack.status, super::PodmanStackStatus::Stopped);
    }
}
