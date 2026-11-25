export function decimalPlaces(step: number): number {
    if (!Number.isFinite(step) || Number.isInteger(step)) {
        return 0;
    }

    const [coefficient, exponent = '0'] = step.toString().toLowerCase().split('e');
    return Math.max(0, (coefficient.split('.')[1]?.length ?? 0) - Number(exponent));
}

/** `value` plus `delta` without floating-point drift. */
export function nudge(value: number, delta: number): number {
    return Number((value + delta).toFixed(Math.max(decimalPlaces(value), decimalPlaces(delta))));
}

/** A typed value: any as precise as the step and `min`, not only the step's multiples; halves away from zero, as the app's. */
export function clampToPrecision(value: number, min: number, max: number, step: number): number {
    const scale = 10 ** Math.max(decimalPlaces(min), decimalPlaces(step));
    // `|| 0`: no -0 to print.
    const rounded = (Math.sign(value) * Math.round(Math.abs(value * scale))) / scale || 0;
    return Math.max(min, Math.min(max, rounded));
}

export function clampToStep(value: number, min: number, max: number, step: number): number {
    return clampToPrecision(min + Math.round((value - min) / step) * step, min, max, step);
}
