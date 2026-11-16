const MOST_DIGITS = 6;
const PROBE = 1.5;
const formats = new Map<string, Intl.NumberFormat>();
const marks = new Map<string, string>();

/** A value as edit text in the language `tag`: its decimal mark, never grouping. */
export function formatInput(tag: string, value: number, digits = MOST_DIGITS): string {
    const key = `${tag}/${digits}`;
    let format = formats.get(key);
    if (!format) {
        format = new Intl.NumberFormat(tag, { maximumFractionDigits: digits, useGrouping: false });
        formats.set(key, format);
    }
    return format.format(value);
}

function decimalMark(tag: string): string {
    let mark = marks.get(tag);
    if (mark === undefined) {
        const parts = new Intl.NumberFormat(tag).formatToParts(PROBE);
        mark = parts.find((part) => part.type === 'decimal')?.value ?? '.';
        marks.set(tag, mark);
    }
    return mark;
}

/**
 * Edit text in the language `tag` as a number: `1,5` is 1.5 in French. `null` for anything else, grouping and
 * another language's decimal mark included: `1.000` in French is not a thousand.
 */
export function parseInput(tag: string, text: string): number | null {
    const mark = decimalMark(tag);
    const written = text.trim().replace('\u2212', '-');
    if (mark !== '.' && written.includes('.')) return null;
    const plain = written.replace(mark, '.');
    return /^[+-]?(\d+\.?\d*|\.\d+)$/.test(plain) ? Number(plain) : null;
}
