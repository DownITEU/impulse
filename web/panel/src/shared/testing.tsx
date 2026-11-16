import { render } from '@testing-library/preact';
import type { ReactNode } from 'react';

import { Texts, texts } from '@/i18n/texts';

/** Renders `ui` as the panel shows it in the language `tag`. */
export const renderIn = (tag: string, ui: ReactNode) =>
    render(<Texts.Provider value={texts(tag)}>{ui}</Texts.Provider>);
