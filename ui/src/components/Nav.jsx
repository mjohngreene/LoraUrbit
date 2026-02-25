import React from 'react';

const tabs = [
  { id: 'dashboard', label: '📊 Dashboard' },
  { id: 'peers', label: '🔗 Peers' },
];

export default function Nav({ page, setPage }) {
  return (
    <nav className="nav">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          className={`nav-btn ${page === tab.id ? 'active' : ''}`}
          onClick={() => setPage(tab.id)}
        >
          {tab.label}
        </button>
      ))}
    </nav>
  );
}
