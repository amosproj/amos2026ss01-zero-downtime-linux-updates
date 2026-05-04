use rust_fsm::*;

state_machine! {
	#[derive(Debug, PartialEq, Eq)]
    #[repr(C)]
    /// The State machine
    pub orchestrator_state(Idle)

	// Idle, Checking, Downloading, Verifying, Installing
	Idle(TimerTriggered) => Checking,
    Checking => {
		UpToDate => Idle,
		UpdateAvailable => Downloading,
	},
    Downloading(Done) => Verifying,
    Verifying(Done) => Installing,
    Installing(Done) => Idle,
}
