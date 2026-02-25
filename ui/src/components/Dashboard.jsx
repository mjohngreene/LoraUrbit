import React, { useState, useEffect } from 'react';
import { scry, subscribe } from '../api';

export default function Dashboard() {
  const [stats, setStats] = useState(null);
  const [error, setError] = useState(null);

  useEffect(() => {
    let mounted = true;

    async function load() {
      try {
        const data = await scry('/stats');
        if (mounted) setStats(data);
      } catch (err) {
        console.error('Failed to load stats:', err);
        if (mounted) setError('Failed to load stats');
      }
    }

    load();

    // Subscribe to uplinks for live counter
    let subId;
    subscribe('/uplinks', (update) => {
      // Re-fetch stats on new uplink
      scry('/stats').then((data) => {
        if (mounted) setStats(data);
      });
    }).then((id) => { subId = id; });

    return () => {
      mounted = false;
    };
  }, []);

  if (error) {
    return <div className="card error">{error}</div>;
  }

  if (!stats) {
    return <div className="card">Loading...</div>;
  }

  const statItems = [
    { label: 'Devices', value: stats['device-count'], icon: '📻' },
    { label: 'Peers', value: stats['peer-count'], icon: '🔗' },
    { label: 'Uplinks', value: stats['uplink-count'], icon: '📶' },
    { label: 'Inbox', value: stats['inbox-count'], icon: '📥' },
    { label: 'Outbox', value: stats['outbox-count'], icon: '📤' },
  ];

  return (
    <div className="dashboard">
      <h2>Dashboard</h2>
      <div className="stat-grid">
        {statItems.map((item) => (
          <div key={item.label} className="stat-card">
            <div className="stat-icon">{item.icon}</div>
            <div className="stat-value">{item.value}</div>
            <div className="stat-label">{item.label}</div>
          </div>
        ))}
      </div>
    </div>
  );
}
