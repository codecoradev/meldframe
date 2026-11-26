// The word a screen shows for a name that is nothing once the app layer has made it safe to show (rule U9): all of it was
// hidden characters, or the core took an empty name (a mode may have one). One word, so no screen words it by itself.

/** What a name that is nothing is called on a screen. */
export const NO_NAME = '(no name)';

/** A name for a label, a title or a sentence: the name the app layer gave, or a word for one that is nothing. */
export const nameOf = (s: string): string => s || NO_NAME;
