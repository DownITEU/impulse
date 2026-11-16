import type { Note } from '@/session/state';

/** The app's refusals and failures, worded; each goes after a few seconds. */
export const Notes = ({ notes }: { notes: Note[] }) => (
    <div
        aria-live="polite"
        className="pointer-events-none absolute inset-x-3.5 bottom-21 z-20 flex flex-col items-center gap-1.5"
    >
        {notes.map((note) => (
            <p
                key={note.id}
                className="max-w-full rounded-[10px] border border-white/10 bg-[#0e1016]/90 px-3 py-2 text-center text-[12px] leading-5 text-(--deck-fg) shadow-[0_12px_40px_-10px_rgba(0,0,0,.65)] backdrop-blur-2xl"
            >
                {note.text}
            </p>
        ))}
    </div>
);
