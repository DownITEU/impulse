import { useEffect, useState } from 'react';

/** Whole seconds since `since` (Unix seconds); ticks once a second, and only while `ticking`. */
export function useElapsed(since: number | null, ticking: boolean): number | null {
    const [now, setNow] = useState(() => Date.now());

    useEffect(() => {
        if (!ticking) return;
        setNow(Date.now());
        const timer = window.setInterval(() => setNow(Date.now()), 1000);
        return () => window.clearInterval(timer);
    }, [ticking]);

    return since === null ? null : Math.max(0, Math.floor(now / 1000) - since);
}
