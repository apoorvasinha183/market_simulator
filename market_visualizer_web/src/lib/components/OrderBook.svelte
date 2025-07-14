
<script lang="ts">
  import type { MarketState } from '../../lib/types';

  export let marketState: MarketState | null;
  export let selectedStockId: string;

  $: orderBook = marketState?.order_books?.[selectedStockId];
  $: bids = orderBook ? Object.entries(orderBook.bids).map(([price, level]) => ({ price: parseFloat(price) / 100, volume: level.total_volume })).sort((a, b) => b.price - a.price) : [];
  $: asks = orderBook ? Object.entries(orderBook.asks).map(([price, level]) => ({ price: parseFloat(price) / 100, volume: level.total_volume })).sort((a, b) => a.price - b.price) : [];

  $: maxVolume = Math.max(
    ...(bids.map(b => b.volume)),
    ...(asks.map(a => a.volume)),
    1
  );

</script>

<div class="order-book-container">
  <div class="header">Order Book</div>
  <table>
    <thead>
      <tr>
        <th>Price (USD)</th>
        <th>Volume</th>
      </tr>
    </thead>
  </table>
  <div class="body asks">
    <table>
      <tbody>
        {#each asks as ask (ask.price)}
          <tr>
            <td class="price ask">{ask.price.toFixed(2)}</td>
            <td>
              {ask.volume.toLocaleString()}
              <div class="depth-bar ask-depth" style="width: {ask.volume / maxVolume * 100}%"></div>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
  <div class="spread">
    <span>Spread: { (asks[0]?.price - bids[0]?.price)?.toFixed(2) || 'N/A' }</span>
  </div>
  <div class="body bids">
    <table>
      <tbody>
        {#each bids as bid (bid.price)}
          <tr>
            <td class="price bid">{bid.price.toFixed(2)}</td>
            <td>
              {bid.volume.toLocaleString()}
              <div class="depth-bar bid-depth" style="width: {bid.volume / maxVolume * 100}%"></div>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</div>

<style>
  .order-book-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    background-color: #1c212e;
    border-radius: 4px;
    border: 1px solid #2a2e39;
    overflow: hidden;
  }

  .header {
    padding: 8px 12px;
    font-weight: bold;
    border-bottom: 1px solid #2a2e39;
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }

  th, td {
    padding: 4px 12px;
    text-align: right;
  }

  th {
    font-size: 12px;
    color: #848e9c;
  }

  .body {
    flex-grow: 1;
    overflow-y: auto;
  }

  .body table {
      position: relative;
  }

  .body tr {
      position: relative;
  }

  .price.bid { color: #26a69a; }
  .price.ask { color: #ef5350; }

  .spread {
    padding: 8px 12px;
    text-align: center;
    font-weight: bold;
    border-top: 1px solid #2a2e39;
    border-bottom: 1px solid #2a2e39;
  }

  .depth-bar {
      position: absolute;
      right: 0;
      top: 0;
      height: 100%;
      opacity: 0.2;
      z-index: 0;
  }

  .bid-depth { background-color: #26a69a; }
  .ask-depth { background-color: #ef5350; }

  td {
      position: relative;
      z-index: 1;
  }
</style>
