
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





    socket.onmessage = (event) => {
        requestAnimationFrame(() => {
            let message;
            try {
                message = JSON.parse(event.data);
            } catch (e) {
                console.error('Failed to parse WebSocket message:', e);
                return;
            }

            switch (message.type) {
                case 'snapshot':
                    marketState.set(message.data);
                    break;
                case 'price_update':
                    marketState.update(currentState => {
                        if (!currentState) return null;
                        return {
                            ...currentState,
                            market_state: {
                                ...currentState.market_state,
                                last_traded_price: message.data.last_traded_price,
                                mid_prices: message.data.mid_prices,
                                spreads: message.data.spreads,
                                cumulative_volume: message.data.cumulative_volume,
                            }
                        };
                    });
                    break;
                case 'candle_update':
                    marketState.update(currentState => {
                        if (!currentState) return null;

                        for (const key in message.data.candle_data) {
                            const newCandles = message.data.candle_data[key];
                            if (!newCandles?.length) continue;

                            if (!currentState.candle_data[key]) {
                                currentState.candle_data[key] = [];
                            }
                            
                            currentState.candle_data[key].push(...newCandles);

                            const stockId = newCandles[0].stock_id;
                            if (stockId) {
                                if (!currentState.price_history[stockId]) {
                                    currentState.price_history[stockId] = [];
                                }
                                
                                for (const candle of newCandles) {
                                    currentState.price_history[stockId].push([candle.timestamp, candle.close]);
                                }
                                
                                const MAX_HISTORY = 1000;
                                if (currentState.price_history[stockId].length > MAX_HISTORY) {
                                    currentState.price_history[stockId] = currentState.price_history[stockId].slice(currentState.price_history[stockId].length - MAX_HISTORY);
                                }
                            }
                        }

                        return { ...currentState };
                    });
                    break;
                case 'orderbook_update':
                    marketState.update(currentState => {
                        if (!currentState) return null;
                        return {
                            ...currentState,
                            market_state: {
                                ...currentState.market_state,
                                order_books: message.data.order_books
                            }
                        };
                    });
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
        });
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
