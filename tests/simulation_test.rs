use market_simulator::{agents::agent_type::AgentType, simulation::orchestra::Orchestra};
use std::{thread, time::Duration};

#[test]
fn test_market_and_agents_interaction() {
    // 1. Setup: Create an orchestra with a MarketMaker and a DumbAgent
    // These two are sufficient to generate and match orders.
    let agent_types = vec![AgentType::MarketMaker, AgentType::DumbMarket];
    let orchestra = Orchestra::new(agent_types, 100, 100);
    let shadow_handle = orchestra.get_shadow_handle();

    // Capture the initial state for comparison later.
    let initial_volume = {
        let state = shadow_handle.read().unwrap();
        // Assuming stock with ID 1 exists from the default universe
        *state.cumulative_volume.get(&1).unwrap_or(&0)
    };

    // 2. Run: Launch the orchestra and let it run for a moment.
    // We run it in a separate thread because orchestra.run() is a blocking call.
    let _orchestra_thread = thread::spawn(move || {
        orchestra.run();
    });

    // Let the simulation run for 2 seconds. This is enough time for the
    // MarketMaker to place limit orders and the DumbAgent to place market orders.
    println!("Running simulation for 2 seconds...");
    thread::sleep(Duration::from_secs(2));

    // Note: In a real-world scenario, we would need a graceful shutdown mechanism
    // for the orchestra. For this test, we'll just detach the thread, as the
    // verification below is the main point. The OS will clean up the threads
    // when the test process exits.
    // In the future, we could add a `shutdown` channel to the Orchestra.

    // 3. Verify: Check the final state of the shadow book.
    let final_volume = {
        let state = shadow_handle.read().unwrap();
        *state.cumulative_volume.get(&1).unwrap_or(&0)
    };

    // 4. Assert: The cumulative volume should have increased, proving trades occurred.
    println!(
        "Initial Volume: {}, Final Volume: {}",
        initial_volume, final_volume
    );
    assert!(
        final_volume > initial_volume,
        "Assertion Failed: The cumulative volume should increase after trades."
    );
}
