import { describe, expect, it } from 'vitest';

import { formatInput, parseInput } from './format-number';

describe('numbers in the app language', () => {
    it('read the decimal mark of the language', () => {
        expect(parseInput('fr', '1,5')).toBe(1.5);
        expect(parseInput('en', '1.5')).toBe(1.5);
        expect(parseInput('en', '-2')).toBe(-2);
    });

    it('refuse grouping and the other language mark', () => {
        expect(parseInput('fr', '1.000')).toBeNull();
        expect(parseInput('en', '1,000')).toBeNull();
        expect(parseInput('en', 'abc')).toBeNull();
    });

    it('write the language mark without grouping', () => {
        expect(formatInput('fr', 1.5)).toBe('1,5');
        expect(formatInput('en', 12345.25)).toBe('12345.25');
    });
});
