mod state;
use state::orchestrator_state;
use std::sync::Arc;
use tokio::{sync::Mutex, time::{Duration, sleep}};

#[tokio::main]
async fn main() {
    println!("Started app...");
    let sm = Arc::new(Mutex::new(orchestrator_state::StateMachine::new()));

    let sm_clone = sm.clone();
    tokio::spawn(async move {
        // Emulate periodic checker timer
        loop {
            println!("tick: sleeping 5s");
            sleep(Duration::from_secs(5)).await;
            println!("tick: awake, acquiring lock");
            let mut guard = sm_clone.lock().await;
            println!("tick: got lock, state = {:?}", guard.state());

            if *guard.state() == orchestrator_state::State::Idle {
                println!("state==Idle, calling consume");
                let res = guard.consume(&orchestrator_state::Input::TimerTriggered);
                drop(guard);

                match res {
                    Ok(_) => println!("transition ok"),
                    Err(e) => {
                        eprintln!("transition impossible: {:?}", e);
                        let guard = sm_clone.lock().await;
                        eprintln!("current state = {:?}", guard.state());
                        drop(guard);
                    }
                }
            }
        }
    });

    tokio::signal::ctrl_c().await.unwrap();
}
