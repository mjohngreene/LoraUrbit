import React, { useState } from 'react';
import Nav from './components/Nav';
import Dashboard from './components/Dashboard';
import Peers from './components/Peers';
import './App.css';

export default function App() {
  const [page, setPage] = useState('dashboard');

  return (
    <div className="app">
      <header className="app-header">
        <h1>📡 LoraUrbit</h1>
        <span className="subtitle">Sovereign LoRaWAN Messaging</span>
      </header>
      <Nav page={page} setPage={setPage} />
      <main className="app-main">
        {page === 'dashboard' && <Dashboard />}
        {page === 'peers' && <Peers />}
      </main>
    </div>
  );
}
