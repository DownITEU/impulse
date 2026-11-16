const formats = new Map<string, Intl.NumberFormat>();

function twoDigits(tag: string): Intl.NumberFormat {
    let format = formats.get(tag);
    if (!format) {
        format = new Intl.NumberFormat(tag, { minimumIntegerDigits: 2, useGrouping: false });
        formats.set(tag, format);
    }
    return format;
}

/** Seconds as `mm:ss`, or `h:mm:ss` from the first hour, in the digits of `tag`. */
export function formatClock(tag: string, seconds: number): string {
    const whole = Math.max(0, Math.floor(seconds));
    const two = twoDigits(tag);
    const clock = `${two.format(Math.floor(whole / 60) % 60)}:${two.format(whole % 60)}`;
    const hours = Math.floor(whole / 3600);
    return hours > 0 ? `${new Intl.NumberFormat(tag).format(hours)}:${clock}` : clock;
}
