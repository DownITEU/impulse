import { createRoot } from 'react-dom/client';

import { connect } from '@/session/store';

import { App } from './app';
import '../index.css';

const root = document.getElementById('app');
if (!root) throw new Error('App root not found.');

if (import.meta.env.DEV && new URLSearchParams(location.search).has('demo')) {
    import('@/session/demo').then(({ play }) => play());
} else {
    void connect();
}

createRoot(root).render(<App />);
