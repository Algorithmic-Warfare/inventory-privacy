import { Link, useLocation } from 'react-router-dom';
import { useSuiClientContext } from '@mysten/dapp-kit';
import { useState, useEffect } from 'react';
import { getLocalAddress, hasLocalSigner } from '../sui/localSigner';

const navItems = [
  { path: '/', label: 'HOME' },
  { path: '/inventory', label: 'CREATE' },
  { path: '/prove', label: 'PROVE' },
  { path: '/operations', label: 'DEPOSIT/WITHDRAW' },
  { path: '/transfer', label: 'TRANSFER' },
  { path: '/on-chain', label: 'ON-CHAIN' },
  { path: '/explorer', label: 'EXPLORER' },
];

function getTheme(): 'light' | 'dark' {
  if (typeof window !== 'undefined') {
    return (document.documentElement.getAttribute('data-theme') as 'light' | 'dark') || 'dark';
  }
  return 'dark';
}

function setTheme(theme: 'light' | 'dark') {
  document.documentElement.setAttribute('data-theme', theme);
  localStorage.setItem('theme', theme);
}

export function Layout({ children }: { children: React.ReactNode }) {
  const location = useLocation();
  const { network } = useSuiClientContext();
  const [theme, setThemeState] = useState<'light' | 'dark'>(getTheme);
  const localAddress = getLocalAddress();

  useEffect(() => {
    const savedTheme = localStorage.getItem('theme') as 'light' | 'dark' | null;
    if (savedTheme) {
      setTheme(savedTheme);
      setThemeState(savedTheme);
    }
  }, []);

  const toggleTheme = () => {
    const newTheme = theme === 'light' ? 'dark' : 'light';
    setTheme(newTheme);
    setThemeState(newTheme);
  };

  return (
    <>
      <header className="header">
        <Link to="/" className="nav-link" style={{ background: 'transparent' }}>
          <span className="header-title">[INVENTORY-PRIVACY]</span>
        </Link>

        <nav className="header-nav">
          {navItems.map((item) => (
            <Link
              key={item.path}
              to={item.path}
              className={`nav-link ${location.pathname === item.path ? 'active' : ''}`}
            >
              {item.label}
            </Link>
          ))}
        </nav>

        <div className="header-actions">
          <span className={`badge ${network === 'localnet' ? 'badge-info' : network === 'testnet' ? 'badge-warning' : 'badge-success'}`}>
            {network?.toUpperCase()}
          </span>
          <button
            onClick={toggleTheme}
            className="btn btn-secondary btn-small"
          >
            [{theme === 'light' ? 'DARK' : 'LIGHT'}]
          </button>
        </div>
      </header>

      {hasLocalSigner() && localAddress && (
        <div className="alert alert-success" style={{ textAlign: 'center', marginBottom: 0 }}>
          [OK] LOCAL SIGNER: {localAddress.slice(0, 8)}...{localAddress.slice(-6)}
        </div>
      )}

      {!hasLocalSigner() && (
        <div className="alert alert-warning" style={{ textAlign: 'center', marginBottom: 0 }}>
          [!!] NO LOCAL SIGNER - Set VITE_SUI_PRIVATE_KEY in .env
        </div>
      )}

      <main className="page-wide" style={{ flex: 1 }}>
        {children}
      </main>

      <footer className="footer">
        INVENTORY PRIVACY POC - ZERO-KNOWLEDGE PROOFS FOR PRIVATE INVENTORY
      </footer>
    </>
  );
}
