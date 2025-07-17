
// src/lib/store.ts

import { writable } from 'svelte/store';
import { v4 as uuidv4 } from 'uuid';

// --- Store State ---

export const marketState = writable(null);
export const portfolio = writable(null);
export const orderConfirmations = writable([]);
export const tradeHistory = writable([]);
export const connectionStatus = writable('disconnected');

// --- WebSocket and Client ID Management ---

let socket: WebSocket | null = null;
let reconnectInterval: number | null = null;

function getClientId(): string {
    let clientId = localStorage.getItem('clientId');
    if (!clientId) {
        clientId = uuidv4();
        localStorage.setItem('clientId', clientId);
    }
    return clientId;
}

function connect() {
    // Clear any existing reconnect interval to prevent multiple attempts
    if (reconnectInterval) {
        clearInterval(reconnectInterval);
        reconnectInterval = null;
    }

    // If a socket already exists and is open or connecting, do nothing
    if (socket && (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)) {
        return;
    }

    // Close any existing socket before creating a new one (if it's not already closed)
    if (socket) socket.close();

    socket = new WebSocket('ws://127.0.0.1:6969/ws');
    connectionStatus.set('connecting');

    socket.onopen = () => {
        console.log('WebSocket connection established.');
        connectionStatus.set('connected');
        // Clear interval on successful connection
        if (reconnectInterval) {
            clearInterval(reconnectInterval);
            reconnectInterval = null;
        }
        // Register the client with the backend
        const clientId = getClientId();
        socket?.send(JSON.stringify({ type: 'Register', payload: { client_id: clientId } }));
    };

    // Message batching for performance
    let updateQueue: any[] = [];
    let batchTimeout: number | null = null;
    
    function processBatchedUpdates() {
        if (updateQueue.length === 0) return;
        
        marketState.update(currentState => {
            if (!currentState) return null;
            
            // Process all queued updates efficiently
            let hasMarketStateChanges = false;
            let hasCandleChanges = false;
            let hasPriceHistoryChanges = false;
            
            for (const update of updateQueue) {
                if (update.market_state) {
                    // Shallow merge market state changes
                    Object.assign(currentState.market_state, update.market_state);
                    hasMarketStateChanges = true;
                }
                
                if (update.candle_data) {
                    for (const key in update.candle_data) {
                        const newCandles = update.candle_data[key];
                        if (!newCandles || newCandles.length === 0) continue;
                        
                        // Efficient append without full array copy
                        if (!currentState.candle_data[key]) {
                            currentState.candle_data[key] = [];
                        }
                        currentState.candle_data[key].push(...newCandles);
                        hasCandleChanges = true;
                        
                        // Update price history with fixed window (last 1000 points to prevent screen pollution)
                        const stockId = newCandles[0].stock_id;
                        if (stockId) {
                            if (!currentState.price_history[stockId]) {
                                currentState.price_history[stockId] = [];
                            }
                            const newHistoryPoints = newCandles.map(c => [c.timestamp, c.close]);
                            
                            // Only append if timestamps are in order, otherwise we need to sort
                            const lastTimestamp = currentState.price_history[stockId].length > 0 
                                ? currentState.price_history[stockId][currentState.price_history[stockId].length - 1][0]
                                : 0;
                            
                            const firstNewTimestamp = newHistoryPoints[0]?.[0] || 0;
                            
                            if (firstNewTimestamp >= lastTimestamp) {
                                // Safe to append - timestamps are in order
                                currentState.price_history[stockId].push(...newHistoryPoints);
                            } else {
                                // Need to merge and sort to maintain chronological order
                                currentState.price_history[stockId].push(...newHistoryPoints);
                                currentState.price_history[stockId].sort((a, b) => a[0] - b[0]);
                            }
                            
                            // Maintain fixed window - keep only last 1000 points
                            const MAX_HISTORY_POINTS = 1000;
                            if (currentState.price_history[stockId].length > MAX_HISTORY_POINTS) {
                                currentState.price_history[stockId] = currentState.price_history[stockId].slice(-MAX_HISTORY_POINTS);
                            }
                            
                            hasPriceHistoryChanges = true;
                        }
                    }
                }
            }
            
            // Only create new state reference if there were actual changes
            if (hasMarketStateChanges || hasCandleChanges || hasPriceHistoryChanges) {
                return { ...currentState }; // Shallow clone to trigger reactivity
            }
            
            return currentState;
        });
        
        updateQueue = [];
        batchTimeout = null;
    }

    socket.onmessage = (event) => {
        const message = JSON.parse(event.data);
        switch (message.type) {
            case 'snapshot':
                // Initial snapshot - set directly
                marketState.set(message.data);
                break;
            case 'update':
                // Batch updates for performance
                updateQueue.push(message.data);
                
                if (!batchTimeout) {
                    batchTimeout = window.setTimeout(processBatchedUpdates, 16); // ~60fps
                }
                break;
            case 'OrderAck':
                orderConfirmations.update(acks => [message.OrderAck, ...acks].slice(0, 10));
                break;
            case 'PortfolioUpdate':
                portfolio.set({ cash: message.cash, holdings: message.holdings });
                break;
            case 'TradeUpdate':
                tradeHistory.update(trades => [message.TradeUpdate, ...trades].slice(0, 50));
                break;
        }
    };

    socket.onclose = () => {
        console.log('WebSocket closed. Reconnecting...');
        connectionStatus.set('disconnected');
        if (!reconnectInterval) {
            reconnectInterval = window.setInterval(connect, 3000);
        }
    };

    socket.onerror = (error) => {
        console.error('WebSocket error:', error);
        connectionStatus.set('disconnected');
        socket?.close();
    };
}

function submitOrder(payload: any) {
    if (socket?.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify({ type: 'SubmitOrder', payload }));
    } else {
        console.error('Cannot submit order, WebSocket is not connected.');
    }
}

// --- Exports ---

export const actions = {
    connect,
    submitOrder,
};
