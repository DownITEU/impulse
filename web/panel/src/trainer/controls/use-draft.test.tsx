import { cleanup, fireEvent, render, screen } from '@testing-library/preact';
import { afterEach, describe, expect, it } from 'vitest';

import { useDraft } from './use-draft';

const Stepper = ({ view }: { view: number }) => {
    const [shown, show] = useDraft(view);
    return (
        <button type="button" onClick={() => show((shown ?? 0) + 1)}>
            {shown}
        </button>
    );
};

afterEach(cleanup);

describe('a draft', () => {
    it('outlives an older echo and ends on its own', () => {
        const { rerender } = render(<Stepper view={0} />);
        const button = screen.getByRole('button');
        fireEvent.click(button);
        fireEvent.click(button);
        rerender(<Stepper view={1} />);
        expect(button.textContent).toBe('2');
        fireEvent.click(button);
        rerender(<Stepper view={3} />);
        rerender(<Stepper view={5} />);
        expect(button.textContent).toBe('5');
    });
});
