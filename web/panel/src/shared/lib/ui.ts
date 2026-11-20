export const cn = (...classes: (string | false | null | undefined)[]): string =>
    classes.filter(Boolean).join(' ');

/** Where the trimmed `query` first shows in `name`, case aside: `[start, end]`, empty for no query, `null` for none. */
export function spot(name: string, query: string): [number, number] | null {
    const wanted = query.trim().toLowerCase();
    const at = name.toLowerCase().indexOf(wanted);
    return at < 0 ? null : [at, at + wanted.length];
}
