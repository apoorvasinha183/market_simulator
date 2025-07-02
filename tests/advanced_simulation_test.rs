use market_simulator::{agents::agent_type::AgentType, simulation::orchestra::Orchestra};
use std::{thread, time::Duration};

/// This test verifies that with only a MarketMaker, liquidity is provided but no trades occur.
#[test]
fn test_market_maker_only_scenario() {
    println!("\n--- Running Test: Market Maker Only ---");
    // 1. Setup: Orchestra with only a MarketMaker.
    let agent_types = vec![AgentType::MarketMaker];
    let orchestra = Orchestra::new(agent_types, 100, 100);
    let shadow_handle = orchestra.get_shadow_handle();

    // Capture initial state.
    let (initial_volume, initial_book_empty) = {
        let state = shadow_handle.read().unwrap();
        let book = state.order_books.get(&1).unwrap();
        (
            *state.cumulative_volume.get(&1).unwrap_or(&0),
            book.bids.is_empty() && book.asks.is_empty(),
        )
    };
    assert!(initial_book_empty, "Book should be empty at genesis.");

    // 2. Run simulation.
    let _orchestra_thread = thread::spawn(move || {
        orchestra.run();
    });
    println!("Running simulation for 2 seconds...");
    thread::sleep(Duration::from_secs(2));

    // 3. Verify final state.
    let (final_volume, final_book_empty) = {
        let state = shadow_handle.read().unwrap();
        let book = state.order_books.get(&1).unwrap();
        (
            *state.cumulative_volume.get(&1).unwrap_or(&0),
            book.bids.is_empty() && book.asks.is_empty(),
        )
    };

    // 4. Assertions.
    println!(
        "Initial Volume: {}, Final Volume: {}",
        initial_volume, final_volume
    );
    assert_eq!(
        final_volume, initial_volume,
        "Assertion Failed: Cumulative volume should be 0 as no trades should have occurred."
    );

    println!(
        "Initial Book Empty: {}, Final Book Empty: {}",
        initial_book_empty, final_book_empty
    );
    assert!(
        !final_book_empty,
        "Assertion Failed: The order book should not be empty; the MarketMaker should have placed orders."
    );
    println!("--- Test Complete: Market Maker Only ---");
}

/// This test verifies that a DumbAgent's buy orders execute against an IPOAgent's sell orders.
#[test]
fn test_ipo_vs_dumb_agent_scenario() {
    println!("\n--- Running Test: IPO vs. Dumb Agent ---");
    // 1. Setup: Orchestra with an IPO agent and a Dumb (market order) agent.
    let agent_types = vec![AgentType::IPO, AgentType::DumbMarket];
    let orchestra = Orchestra::new(agent_types, 100, 100);
    let shadow_handle = orchestra.get_shadow_handle();

    // Capture initial state.
    let initial_volume = {
        let state = shadow_handle.read().unwrap();
        *state.cumulative_volume.get(&1).unwrap_or(&0)
    };

    // 2. Run simulation.
    let _orchestra_thread = thread::spawn(move || {
        orchestra.run();
    });
    println!("Running simulation for 5 seconds...");
    thread::sleep(Duration::from_secs(5));

    // 3. Verify final state.
    let final_volume = {
        let state = shadow_handle.read().unwrap();
        *state.cumulative_volume.get(&1).unwrap_or(&0)
    };

    // 4. Assertions.
    println!(
        "Initial Volume: {}, Final Volume: {}",
        initial_volume, final_volume
    );
    assert!(
        final_volume > initial_volume,
        "Assertion Failed: Cumulative volume should increase as the DumbAgent buys from the IPOAgent."
    );
    println!("--- Test Complete: IPO vs. Dumb Agent ---");
}
