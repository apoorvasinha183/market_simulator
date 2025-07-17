
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

    // Immediate processing with smart throttling - no batching barriers
    let lastUpdateTime = 0;
    let isProcessing = false;
    
    function processUpdateImmediately(updateData: any) {
        if (isProcessing) return; // Skip if already processing
        
        const now = performance.now();
        // Throttle to max 120fps (8.33ms) to prevent overwhelming but stay responsive
        if (now - lastUpdateTime < 8.33) return;
        
        isProcessing = true;
        lastUpdateTime = now;
        
        marketState.update(currentState => {
            if (!currentState) {
                isProcessing = false;
                return null;
            }
            
            let needsUpdate = false;
            
            // Process single update immediately
            if (updateData.market_state) {
                // Direct property assignment - fastest possible
                for (const key in updateData.market_state) {
                    if (currentState.market_state[key] !== updateData.market_state[key]) {
                        currentState.market_state[key] = updateData.market_state[key];
                        needsUpdate = true;
                    }
                }
            }
            
            if (updateData.candle_data) {
                for (const key in updateData.candle_data) {
                    const newCandles = updateData.candle_data[key];
                    if (!newCandles?.length) continue;
                    
                    // Initialize arrays if needed
                    if (!currentState.candle_data[key]) {
                        currentState.candle_data[key] = [];
                    }
                    
                    // Direct append candles
                    const existingCandles = currentState.candle_data[key];
                    const startLength = existingCandles.length;
                    
                    // Direct array extension - fastest method
                    for (let i = 0; i < newCandles.length; i++) {
                        existingCandles[startLength + i] = newCandles[i];
                    }
                    existingCandles.length = startLength + newCandles.length;
                    
                    // Update price history efficiently
                    const stockId = newCandles[0].stock_id;
                    if (stockId) {
                        if (!currentState.price_history[stockId]) {
                            currentState.price_history[stockId] = [];
                        }
                        
                        const priceHistory = currentState.price_history[stockId];
                        const lastTimestamp = priceHistory.length > 0 ? priceHistory[priceHistory.length - 1][0] : 0;
                        
                        // Fast path: append in order
                        let needsSort = false;
                        for (const candle of newCandles) {
                            if (candle.timestamp >= lastTimestamp) {
                                priceHistory.push([candle.timestamp, candle.close]);
                            } else {
                                priceHistory.push([candle.timestamp, candle.close]);
                                needsSort = true;
                            }
                        }
                        
                        // Only sort if necessary
                        if (needsSort) {
                            priceHistory.sort((a, b) => a[0] - b[0]);
                        }
                        
                        // Maintain window efficiently
                        const MAX_HISTORY = 1000;
                        if (priceHistory.length > MAX_HISTORY) {
                            const excess = priceHistory.length - MAX_HISTORY;
                            priceHistory.splice(0, excess);
                        }
                    }
                    
                    needsUpdate = true;
                }
            }
            
            isProcessing = false;
            
            // Only trigger reactivity if there were actual changes
            return needsUpdate ? { ...currentState } : currentState;
        });
    }

    socket.onmessage = (event) => {
        // Fast path: parse JSON only once
        let message;
        try {
            message = JSON.parse(event.data);
        } catch (e) {
            console.error('Failed to parse WebSocket message:', e);
            return;
        }

        switch (message.type) {
            case 'snapshot':
                // Initial snapshot - set directly
                marketState.set(message.data);
                break;
            case 'update':
                // Process updates immediately with smart throttling - no batching barriers
                processUpdateImmediately(message.data);
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
