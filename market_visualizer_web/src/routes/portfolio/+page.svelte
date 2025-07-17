
<script lang="ts">
  import { portfolio, marketState } from '$lib/store';
  import { goto } from '$app/navigation';

  // Reactive declaration for current prices
  $: currentPrices = $marketState?.market_state?.last_traded_price || {};

  // Reactive declaration for calculated portfolio values
  $: calculatedHoldings = $portfolio?.holdings ? Object.values($portfolio.holdings).map(holding => {
    const currentPrice = currentPrices[holding.stock_id] || 0;
    const holdingTotalValue = holding.quantity * currentPrice;
    return { ...holding, currentPrice, holdingTotalValue };
  }) : [];

  $: portfolioTotalValue = ($portfolio?.cash || 0) + calculatedHoldings.reduce((sum, h) => sum + h.holdingTotalValue, 0);

  function formatCurrency(value: number | undefined) {
    if (value === undefined || isNaN(value)) return 'N/A';
    return value.toLocaleString('en-US', { style: 'currency', currency: 'USD' });
  }

  function goHome() {
    goto('/');
  }
</script>

<div class="portfolio-page">
  <button class="back-button" on:click={goHome}>← Back to Home</button>
  <h1>My Portfolio</h1>

  <div class="summary-card">
    <h2>Portfolio Value: {formatCurrency(portfolioTotalValue)}</h2>
    <p>Cash: {formatCurrency($portfolio?.cash)}</p>
  </div>

  <div class="holdings-table">
    <h2>Holdings</h2>
    <table>
      <thead>
        <tr>
          <th>Symbol</th>
          <th>Quantity</th>
          <th>Cost Basis</th>
          <th>Current Price</th>
          <th>Total Value</th>
        </tr>
      </thead>
      <tbody>
        {#if calculatedHoldings.length > 0}
          {#each calculatedHoldings as holding}
            <tr>
              <td>{holding.stock_id}</td>
              <td>{holding.quantity}</td>
              <td>{formatCurrency(holding.cost_basis)}</td>
              <td>{formatCurrency(holding.currentPrice)}</td>
              <td>{formatCurrency(holding.holdingTotalValue)}</td>
            </tr>
          {/each}
        {:else}
          <tr>
            <td colspan="5">No holdings yet.</td>
          </tr>
        {/if}
      </tbody>
    </table>
  </div>
</div>

<style>
  .portfolio-page {
    padding: 2rem;
  }
  .back-button {
    background-color: #333;
    color: #fff;
    border: none;
    padding: 10px 15px;
    border-radius: 5px;
    cursor: pointer;
    margin-bottom: 1rem;
    font-size: 1em;
  }
  .back-button:hover {
    background-color: #555;
  }
  .summary-card {
    background-color: #1c212e;
    padding: 1.5rem;
    border-radius: 8px;
    margin-bottom: 2rem;
  }
  .holdings-table table {
    width: 100%;
    border-collapse: collapse;
  }
  .holdings-table th, .holdings-table td {
    padding: 0.75rem 1rem;
    border-bottom: 1px solid #2a2e39;
  }
</style>
