import { describe, expect, it } from 'vitest';

import { clampToPrecision, clampToStep } from './step';

describe('slider values', () => {
    it('snap a drag to the step', () => {
        expect(clampToStep(150, 0, 1000, 20)).toBe(160);
        expect(clampToStep(2.2, 1, 5, 0.5)).toBe(2);
    });

    it('keep a typed value at the step precision', () => {
        expect(clampToPrecision(150.4, 0, 1000, 20)).toBe(150);
        expect(clampToPrecision(2.34, 1, 5, 0.5)).toBe(2.3);
        expect(clampToPrecision(9, 1, 5, 0.5)).toBe(5);
    });

    it('round a half away from zero, as the app does', () => {
        expect(clampToPrecision(1.15, 0, 5, 0.1)).toBe(1.2);
        expect(clampToPrecision(-1.15, -5, 5, 0.1)).toBe(-1.2);
        expect(clampToPrecision(-0.04, -5, 5, 0.1)).toBe(0);
    });
});
