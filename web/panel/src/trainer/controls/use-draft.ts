import { useCallback, useEffect, useRef, useState } from 'react';

/** How long a control keeps showing what it sent while the app has not shown it back. */
const LIFE_MS = 1500;

/**
 * What a control shows: its own draft, set when the user acts, else the app's `view`. A draft ends when `view`
 * equals it (an older echo does not end a newer draft) or `LIFE_MS` after it was set; a held one (a drag) lasts
 * until it is set again unheld.
 */
export function useDraft<T>(
    view: T | undefined,
): [T | undefined, (next: T, hold?: boolean) => void] {
    const [draft, setDraft] = useState<{ value: T; held: boolean } | null>(null);
    const timer = useRef<number | undefined>(undefined);

    const show = useCallback((value: T, held = false) => {
        setDraft({ value, held });
        window.clearTimeout(timer.current);
        if (!held) timer.current = window.setTimeout(() => setDraft(null), LIFE_MS);
    }, []);

    useEffect(() => {
        if (draft && !draft.held && draft.value === view) {
            window.clearTimeout(timer.current);
            setDraft(null);
        }
    }, [draft, view]);
    useEffect(() => () => window.clearTimeout(timer.current), []);

    return [draft ? draft.value : view, show];
}
