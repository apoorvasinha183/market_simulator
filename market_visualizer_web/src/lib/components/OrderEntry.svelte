<script lang="ts">
  import { actions } from '$lib/store';

  let side: 'Buy' | 'Sell' = 'Buy';
  let orderType: 'Market' | 'Limit' = 'Market';
  let volume: number = 100;
  let price: number | undefined = undefined;
  let stockId: number = 1;

  function handleSubmit() {
    const payload = {
      stock_id: stockId,
      side,
      order_type: orderType,
      volume,
      price: orderType === 'Limit' ? price : undefined,
    };
    actions.submitOrder(payload);
  }
</script>

<div class="order-entry-container">
  <div class="header">Order Entry</div>
  <form on:submit|preventDefault={handleSubmit} class="form-grid">
    <div class="form-group">
      <label for="stock-id">Stock ID</label>
      <input id="stock-id" type="number" bind:value={stockId} min="1">
    </div>
    <div class="form-group">
      <label for="side">Side</label>
      <select id="side" bind:value={side}>
        <option value="Buy">Buy</option>
        <option value="Sell">Sell</option>
      </select>
    </div>
    <div class="form-group">
      <label for="order-type">Order Type</label>
      <select id="order-type" bind:value={orderType}>
        <option value="Market">Market</option>
        <option value="Limit">Limit</option>
      </select>
    </div>
    <div class="form-group">
      <label for="volume">Volume</label>
      <input id="volume" type="number" bind:value={volume} min="1">
    </div>
    {#if orderType === 'Limit'}
      <div class="form-group">
        <label for="price">Price</label>
        <input id="price" type="number" bind:value={price} min="0.01" step="0.01">
      </div>
    {/if}
    <div class="form-group submit-group">
      <button type="submit" class="submit-button">Submit Order</button>
    </div>
  </form>
</div>

<style>
  .order-entry-container {
    background-color: #1c212e;
    border-radius: 4px;
    border: 1px solid #2a2e39;
    padding: 12px;
  }
  .header {
    font-weight: bold;
    margin-bottom: 12px;
  }
  .form-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
    gap: 1rem;
  }
  .form-group {
    display: flex;
    flex-direction: column;
  }
  .submit-group {
    justify-content: flex-end;
  }
  label {
    margin-bottom: 0.5rem;
    font-size: 0.875rem;
  }
  input, select {
    padding: 0.5rem;
    background-color: #2a2e39;
    border: 1px solid #4a4e59;
    color: #d1d4dc;
    border-radius: 4px;
  }
  .submit-button {
    padding: 0.5rem 1rem;
    background-color: #26a69a;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    align-self: flex-end;
  }
</style>