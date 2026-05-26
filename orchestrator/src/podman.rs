// Control Podman
// This assumes full ownership of the Podman instance

// This module mainly works using the "podman compose" CLI, and sources the files from an OCI registry
// To remember the version of the respective stack, small marker files are kept in the docker compose cache

use std::{
    collections::BTreeMap,
    error::Error,
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::Deserialize;
use tokio::{io::AsyncReadExt, process::Command};

type PodmanErr<R> = Result<R, Box<dyn std::error::Error>>;

// Ensure only a single instance of this ever exists
static INSTANCE: Mutex<Option<Podman<RealPodmanRunner>>> = Mutex::new(Option::Some(Podman {
    version_cache: BTreeMap::new(),
    phantom: PhantomData,
}));

struct Podman<R: PodmanRunner> {
    version_cache: BTreeMap<String, String>,
    phantom: PhantomData<R>,
}

#[async_trait::async_trait]
trait PodmanRunner {
    async fn run_podman(args: &[&str]) -> PodmanErr<Vec<u8>>;
    async fn try_write_version(item: &ComposeLsResultItem, ver: &str) -> PodmanErr<()>;
    async fn try_read_version(item: &ComposeLsResultItem) -> PodmanErr<String>;
}

impl Podman<RealPodmanRunner> {
    pub fn take() -> PodmanErr<Self> {
        INSTANCE
            .lock()?
            .take()
            .ok_or("Cannot create multiple Podman instances".into())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ComposeLsResultItem<'a> {
    name: &'a str,
    status: &'a str,
    config_files: &'a str,
}

impl<R: PodmanRunner> Podman<R> {
    pub async fn stacks<'a>(&'a mut self) -> PodmanErr<Vec<PodmanStack<'a, R>>> {
        let cmd_output = R::run_podman(&["compose", "ls", "--all", "--format", "json"]).await?;

        let mut cmd_items: Vec<ComposeLsResultItem> = serde_json::from_slice(&cmd_output)?;
        cmd_items.sort_by_key(|item| item.name); // Usually already sorted

        let output_futs = cmd_items.iter().map(async |item| {
            let status = match item.status {
                s if s.contains(',') => PodmanStackStatus::Ambiguous,
                s if s.starts_with("running") => PodmanStackStatus::Running,
                _ => PodmanStackStatus::Stopped,
            };

            // Retrieve version from either cache or marker file
            let version = match self.version_cache.get(item.name) {
                Some(v) => Some(v.to_owned()),
                None => R::try_read_version(item).await.ok(),
            };

            PodmanStack {
                id: item.name.to_owned(),
                status,
                version,
                phantom: PhantomData,
            }
        });

        let output = futures::future::join_all(output_futs).await;

        self.version_cache.extend(
            output
                .iter()
                .filter_map(|ps| ps.version.to_owned().map(|v| (ps.id.to_owned(), v))),
        );

        // Remove old keys from the version cache, needs sorted items
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
    pub async fn pull<'a>(&mut self, source: StackSource<'a>) -> PodmanErr<()> {
        R::run_podman(&["compose", "-f", source.0, "pull", "--quiet"]).await?;
        Ok(())
    }

    // Create a new stack, start it with start()
    pub async fn create<'a, 'b>(
        &'a mut self,
        id: String,
        source: StackSource<'b>,
        version: String,
    ) -> PodmanErr<PodmanStack<'a, R>> {
        if self.version_cache.contains_key(&id) {
            return Err("Stack with this ID already exists, consider destroying it first".into());
        }

        #[rustfmt::skip]
        R::run_podman(&["compose", "-p", &id, "-f", source.0, "create",
            "--quiet-pull", "--no-build", "--yes"]).await?;

        // Try saving the stack version to the compose cache folder
        if let Err(e) = (async || -> PodmanErr<()> {
            #[rustfmt::skip]
            let ls_output = R::run_podman(&["compose", "ls",
                "--all", "--format", "json", "--filter", &format!("name=^{id}$")]).await?;
            let ls_items: Vec<ComposeLsResultItem> = serde_json::from_slice(&ls_output)?;

            if ls_items.len() != 1 {
                return Err("Stack not found".into());
            }

            R::try_write_version(&ls_items[0], &version).await?;
            Ok(())
        })()
        .await
        {
            log::info!("Failed to save stack version: {e}");
        }

        self.version_cache.insert(id.clone(), version.clone());

        Ok(PodmanStack {
            id,
            status: PodmanStackStatus::Stopped,
            version: Some(version),
            phantom: PhantomData,
        })
    }

    // Remove stopped containers, unused images, volumes and cache entries
    pub async fn cleanup<'a>(&'a mut self) -> PodmanErr<()> {
        R::run_podman(&["system", "prune", "--all", "--force", "--volumes"]).await?;

        // Compose cache needs some extra attention
        let ls_output = R::run_podman(&["compose", "ls", "--all", "--format", "json"]).await?;
        let ls_items: Vec<ComposeLsResultItem> = serde_json::from_slice(&ls_output)?;

        let (base_dirs, mut ids): (Vec<_>, Vec<_>) = ls_items
            .iter()
            .map(|item| item.config_files.split(','))
            .flatten()
            .filter_map(|file_path| Path::new(file_path.trim()).parent())
            .filter_map(|p| match (p.parent(), p.file_name()) {
                (Some(x), Some(y)) => Some((x, y)),
                _ => None,
            })
            .unzip();

        if base_dirs.len() == 0 {
            return Ok(());
        }

        let cache_dir_path = base_dirs[0].canonicalize()?;
        if cache_dir_path.components().count() < 3 || base_dirs.iter().any(|&x| x != base_dirs[0]) {
            return Err(
                format!("Not sure about Compose cache folder ({:?})", cache_dir_path).into(),
            );
        }

        ids.sort();

        let mut cache_dir = tokio::fs::read_dir(&cache_dir_path).await?;
        let mut deletion_list = Vec::new();
        while let Ok(Some(d)) = cache_dir.next_entry().await {
            if ids.binary_search(&d.file_name().as_os_str()).is_err() {
                // Cache folder has no reference from a Compose stack
                deletion_list.push(d.path());
            }
        }

        println!("Deletion list: {:?}", deletion_list);
        for item in deletion_list {
            // Treat deletion failures as non-fatal
            let _ = tokio::fs::remove_dir_all(&item).await;
        }

        Ok(())
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

struct PodmanStack<'a, R: PodmanRunner> {
    id: String,
    status: PodmanStackStatus,
    version: Option<String>,
    phantom: PhantomData<(&'a (), R)>,
}

#[derive(PartialEq, Eq, Debug)]
enum PodmanStackStatus {
    Stopped,
    Ambiguous,
    Running,
}

impl<'a, R: PodmanRunner> PodmanStack<'a, R> {
    pub async fn start(&mut self) -> PodmanErr<()> {
        #[rustfmt::skip]
        R::run_podman(&["compose", "-p", &self.id, "start",
            "--wait", "--wait-timeout", "30"]).await?;
        Ok(())
    }

    pub async fn stop(&mut self) -> PodmanErr<()> {
        R::run_podman(&["compose", "-p", &self.id, "stop", "--timeout", "30"]).await?;
        Ok(())
    }

    pub async fn destroy(self) -> PodmanErr<()> {
        R::run_podman(&["compose", "-p", &self.id, "down", "--timeout", "30"]).await?;
        Ok(())
    }
}

struct RealPodmanRunner;

impl RealPodmanRunner {
    fn infer_version_file_path(ls_item: &ComposeLsResultItem) -> PodmanErr<PathBuf> {
        let mut folders = ls_item
            .config_files
            .split(',')
            .map(|s| Path::new(s).parent());
        let config_folder = match folders.next() {
            Some(Some(first)) if folders.all(|x| x == Some(first)) => first,
            _ => return Err("Not sure about config folder".into()),
        };
        Ok(config_folder.join("version"))
    }
}

#[async_trait::async_trait]
impl PodmanRunner for RealPodmanRunner {
    async fn run_podman(args: &[&str]) -> PodmanErr<Vec<u8>> {
        match Command::new("/bin/podman").args(args).output().await {
            Err(_) => Err(Box::from("Could not find Podman CLI")),
            Ok(o) if !o.status.success() => Err(match String::from_utf8(o.stderr) {
                Ok(msg) => Box::from(format!("Podman error: {}", msg)),
                _ => Box::from("Podman returned an error"),
            }),
            Ok(o) => Ok(o.stdout),
        }
    }

    async fn try_write_version(item: &ComposeLsResultItem, ver: &str) -> PodmanErr<()> {
        let path = Self::infer_version_file_path(item)?;
        tokio::fs::write(path, ver.as_bytes()).await?;
        Ok(())
    }

    async fn try_read_version(item: &ComposeLsResultItem) -> PodmanErr<String> {
        // Try retrieving the stack version by looking in the compose cache folder
        let path = Self::infer_version_file_path(item)?;
        let file = tokio::fs::File::open(path).await?;
        let mut output = String::new();
        file.take(512).read_to_string(&mut output).await?;
        Ok(output)
    }
}

#[cfg(test)]
mod test {
    use std::{collections::BTreeMap, marker::PhantomData};

    struct MockPodmanRunner;

    #[async_trait::async_trait]
    impl super::PodmanRunner for MockPodmanRunner {
        async fn run_podman(args: &[&str]) -> super::PodmanErr<Vec<u8>> {
            println!("Running podman {:?}", args);
            match args {
                ["compose", "ls", ..] => Ok(r#"[{"Name":"bbbb","Status":"created(13), exited(2)","ConfigFiles":"/test/compose.yaml"},{"Name":"aaaa","Status":"running(13)","ConfigFiles":"/test/compose.yaml"}]"#.into()),
                ["compose", ..] if args.contains(&"create") => Ok(Vec::new()),
                _ => Err("Unknown command".into()),
            }
        }

        async fn try_write_version(
            _item: &super::ComposeLsResultItem,
            _ver: &str,
        ) -> super::PodmanErr<()> {
            Ok(())
        }

        async fn try_read_version(_item: &super::ComposeLsResultItem) -> super::PodmanErr<String> {
            Ok("1.2.3".to_owned())
        }
    }

    #[tokio::test]
    async fn test_stack_create() {
        let mut podman: super::Podman<MockPodmanRunner> = super::Podman {
            version_cache: BTreeMap::new(),
            phantom: PhantomData,
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
        assert_eq!(stack.version.unwrap(), "1.2.3");
        assert_eq!(stack.status, super::PodmanStackStatus::Stopped);

        let (k, v) = podman.version_cache.first_key_value().unwrap();
        assert_eq!(k, "test");
        assert_eq!(v, "1.2.3");
    }

    #[tokio::test]
    async fn test_stack_list() {
        let mut podman: super::Podman<MockPodmanRunner> = super::Podman {
            version_cache: BTreeMap::new(),
            phantom: PhantomData,
        };
        podman
            .version_cache
            .insert("bbbb".to_owned(), "test".to_owned());

        let stacks = podman.stacks().await.unwrap();
        assert_eq!(stacks.len(), 2);

        assert_eq!(stacks[0].id, "aaaa");
        assert_eq!(stacks[0].version.as_ref().unwrap(), "1.2.3");
        assert_eq!(stacks[0].status, super::PodmanStackStatus::Running);

        assert_eq!(stacks[1].id, "bbbb");
        assert_eq!(stacks[1].version.as_ref().unwrap(), "test");
        assert_eq!(stacks[1].status, super::PodmanStackStatus::Ambiguous);

        assert_eq!(podman.version_cache.len(), 2)
    }

    #[tokio::test]
    #[ignore = "Must have Podman + Compose installed and completely clean"]
    async fn test_real_pull_create() {
        let mut p = super::Podman::take().unwrap();
        let test_source =
            super::StackSource::new("oci://docker.io/openvidu/local-meet:3.7.0").unwrap();

        assert_eq!(p.stacks().await.unwrap().len(), 0, "Please clean env");

        p.pull(test_source).await.unwrap();
        assert_eq!(p.stacks().await.unwrap().len(), 0);
        assert_eq!(p.version_cache.len(), 0);

        let stack = p
            .create("ccc".to_owned(), test_source, "1".to_owned())
            .await
            .unwrap();
        assert_eq!(stack.id, "ccc");
        assert_eq!(stack.status, super::PodmanStackStatus::Stopped);
        assert_eq!(stack.version.unwrap(), "1");
        assert_eq!(p.version_cache.len(), 1);

        let mut stacks = p.stacks().await.unwrap();
        assert_eq!(stacks.len(), 1);
        assert_eq!(stacks[0].id, "ccc");
        assert_eq!(stacks[0].status, super::PodmanStackStatus::Stopped);

        stacks.pop().unwrap().destroy().await.unwrap();

        assert_eq!(p.stacks().await.unwrap().len(), 0);
        assert_eq!(p.version_cache.len(), 0);
    }
}
