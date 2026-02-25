import React, { useState, useEffect } from 'react';
import { scry, poke, subscribe } from '../api';

export default function Peers() {
  const [peers, setPeers] = useState([]);
  const [error, setError] = useState(null);

  // Register peer form
  const [peerShip, setPeerShip] = useState('');
  const [peerAddr, setPeerAddr] = useState('');
  const [peerStatus, setPeerStatus] = useState('');

  // Set identity form
  const [myAddr, setMyAddr] = useState('');
  const [identityStatus, setIdentityStatus] = useState('');

  useEffect(() => {
    let mounted = true;

    async function load() {
      try {
        const data = await scry('/peers');
        if (mounted) setPeers(data);
      } catch (err) {
        console.error('Failed to load peers:', err);
        if (mounted) setError('Failed to load peers');
      }
    }

    load();

    // Subscribe for live peer updates
    subscribe('/peers', (update) => {
      if (update.type === 'peer-registered') {
        // Re-fetch full peer list
        scry('/peers').then((data) => {
          if (mounted) setPeers(data);
        });
      }
    });

    return () => { mounted = false; };
  }, []);

  async function handleRegisterPeer(e) {
    e.preventDefault();
    setPeerStatus('');
    const ship = peerShip.startsWith('~') ? peerShip : `~${peerShip}`;
    try {
      await poke({
        action: 'register-peer',
        ship,
        'dev-addr': peerAddr,
      });
      setPeerStatus('✅ Peer registered');
      setPeerShip('');
      setPeerAddr('');
      // Re-fetch peers
      const data = await scry('/peers');
      setPeers(data);
    } catch (err) {
      console.error('Failed to register peer:', err);
      setPeerStatus('❌ Failed to register peer');
    }
  }

  async function handleSetIdentity(e) {
    e.preventDefault();
    setIdentityStatus('');
    try {
      await poke({
        action: 'set-identity',
        'dev-addr': myAddr,
      });
      setIdentityStatus('✅ Identity set');
    } catch (err) {
      console.error('Failed to set identity:', err);
      setIdentityStatus('❌ Failed to set identity');
    }
  }

  function formatTime(epochSec) {
    if (!epochSec) return '—';
    const d = new Date(epochSec * 1000);
    return d.toLocaleString();
  }

  return (
    <div className="peers">
      <h2>Peers</h2>

      {/* Set Identity */}
      <div className="card">
        <h3>Set My DevAddr</h3>
        <form onSubmit={handleSetIdentity} className="form-row">
          <input
            type="text"
            placeholder="DevAddr (e.g. 260B1234)"
            value={myAddr}
            onChange={(e) => setMyAddr(e.target.value)}
            required
          />
          <button type="submit">Set Identity</button>
        </form>
        {identityStatus && <div className="status-msg">{identityStatus}</div>}
      </div>

      {/* Register Peer */}
      <div className="card">
        <h3>Register Peer</h3>
        <form onSubmit={handleRegisterPeer} className="form-row">
          <input
            type="text"
            placeholder="Ship (e.g. ~bus)"
            value={peerShip}
            onChange={(e) => setPeerShip(e.target.value)}
            required
          />
          <input
            type="text"
            placeholder="DevAddr (e.g. 01AB5678)"
            value={peerAddr}
            onChange={(e) => setPeerAddr(e.target.value)}
            required
          />
          <button type="submit">Register</button>
        </form>
        {peerStatus && <div className="status-msg">{peerStatus}</div>}
      </div>

      {/* Peer List */}
      <div className="card">
        <h3>Known Peers</h3>
        {error && <div className="error">{error}</div>}
        {peers.length === 0 ? (
          <p className="empty">No peers registered yet.</p>
        ) : (
          <table className="peer-table">
            <thead>
              <tr>
                <th>Ship</th>
                <th>DevAddr</th>
                <th>Status</th>
                <th>Last Seen</th>
              </tr>
            </thead>
            <tbody>
              {peers.map((p, i) => (
                <tr key={i}>
                  <td className="mono">{p.ship}</td>
                  <td className="mono">{p['dev-addr']}</td>
                  <td>
                    <span className={`status-dot ${p.status}`} />
                    {p.status}
                  </td>
                  <td>{formatTime(p['last-seen'])}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}
