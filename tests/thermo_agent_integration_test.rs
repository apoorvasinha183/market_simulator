use market_simulator::{agents::agent_type::AgentType, simulation::orchestra::Orchestra};
use std::{thread, time::Duration, process::{Command, Child}, io::{BufReader, BufRead}};

fn setup_sentiment_service() -> Child {
    // Ensure stock.csv is in the current working directory for the sentiment service
    // In a real scenario, you might copy it or specify its path.

    // Build the sentiment_service binary
    let build_status = Command::new("cargo")
        .args(&["build", "--bin", "sentiment_service"])
        .current_dir("sentiment_service/sentiment")
        .status()
        .expect("Failed to build sentiment_service");
    assert!(build_status.success(), "Failed to build sentiment_service");

    // Start the sentiment_service as a child process
    let mut child = Command::new("cargo")
        .args(&["run", "--bin", "sentiment_service", "--", "../../stock.csv"])
        .current_dir("sentiment_service/sentiment")
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to start sentiment_service");

    // Wait for the service to indicate it has started listening
    let stdout = child.stdout.take().expect("Failed to capture stdout");
    let reader = BufReader::new(stdout);
    for line in reader.lines() {
        let line = line.expect("Failed to read line from stdout");
        println!("[sentiment_service output] {}", line);
        if line.contains("Sentiment microservice starting...") {
            break;
        }
    }
    thread::sleep(Duration::from_millis(500)); // Give it a little more time to bind ports
    child
}

fn teardown_sentiment_service(mut child: Child) {
    println!("Terminating sentiment_service...");
    child.kill().expect("Failed to kill sentiment_service");
    child.wait().expect("Failed to wait for sentiment_service");
    println!("sentiment_service terminated.");
}

#[test]
fn test_thermo_agent_generates_volume() {
    println!("\n--- Running Test: ThermoAgent Generates Volume ---");

    let sentiment_service_child = setup_sentiment_service();

    // 1. Setup: Orchestra with a MarketMaker and a ThermoAgent with high initial temperature
    let agent_types = vec![
        AgentType::MarketMaker,
        AgentType::Thermodynamic {
            initial_temperature: 0.8, // High temperature to ensure activity
            specific_heat: 0.5,
            initial_chemical_potential: 0.0,
        },
    ];
    let orchestra = Orchestra::new(agent_types, 100, 100);
    let shadow_handle = orchestra.get_shadow_handle();

    // Capture the initial state for comparison later.
    let initial_volume = {
        let state = shadow_handle.read().unwrap();
        // Assuming stock with ID 1 exists from the default universe
        *state.cumulative_volume.get(&1).unwrap_or(&0)
    };

    // 2. Run: Launch the orchestra and let it run for a moment.
    let _orchestra_thread = thread::spawn(move || {
        orchestra.run();
    });

    // Let the simulation run for a few seconds. This should be enough time for the
    // MarketMaker to place limit orders and the ThermoAgent to place market orders.
    println!("Running simulation for 5 seconds...");
    let mut elapsed_time = 0;
    let max_wait_time = 10; // seconds
    let check_interval = 100; // milliseconds

    let mut volume_increased = false;
    loop {
        let current_volume = {
            let state = shadow_handle.read().unwrap();
            *state.cumulative_volume.get(&1).unwrap_or(&0)
        };

        if current_volume > initial_volume {
            volume_increased = true;
            println!("Volume increased. Breaking loop.");
            break;
        }

        if elapsed_time >= max_wait_time * 1000 {
            println!("Max wait time reached, volume did not increase sufficiently.");
            break;
        }

        thread::sleep(Duration::from_millis(check_interval));
        elapsed_time += check_interval;
    }

    // 3. Verify: Check the final state of the shadow book.
    let final_volume = {
        let state = shadow_handle.read().unwrap();
        *state.cumulative_volume.get(&1).unwrap_or(&0)
    };

    // 4. Assert: The cumulative volume should have increased, proving trades occurred.
    println!(
        "Initial Volume: {}, Final Volume: {}",
        initial_volume,
        final_volume
    );
    assert!(
        volume_increased,
        "Assertion Failed: The cumulative volume should increase after trades. Expected > {}, got {}",
        initial_volume,
        final_volume
    );
    println!("--- Test Complete: ThermoAgent Generates Volume ---");

    teardown_sentiment_service(sentiment_service_child);
}