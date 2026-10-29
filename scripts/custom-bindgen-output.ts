/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/

function stripAnsiEscapeSequences(value: string) {
    let result = "";

    for (let index = 0; index < value.length; index += 1) {
        if (value.charCodeAt(index) !== 0x1b || value[index + 1] !== "[") {
            result += value[index];
            continue;
        }

        index += 2;
        while (index < value.length) {
            const characterCode = value.charCodeAt(index);
            if (characterCode >= 0x40 && characterCode <= 0x7e) {
                break;
            }
            index += 1;
        }
    }

    return result;
}

export function createCustomBindgenPatchOutputFilter(enabled: boolean) {
    let suppressingPatchHelpContinuation = false;

    return (line: string) => {
        if (!enabled) {
            return false;
        }

        const normalizedLine = stripAnsiEscapeSequences(line).replace(
            /\r?\n$/,
            "",
        );

        if (suppressingPatchHelpContinuation) {
            if (/^\s/.test(normalizedLine)) {
                return true;
            }
            suppressingPatchHelpContinuation = false;
        }

        if (
            /^\s*(?:Locking\b|Adding\b|warning: patch\b)/.test(normalizedLine)
        ) {
            return true;
        }

        if (/^\s*help:.*\bpatch(?:ed|ing)?\b/i.test(normalizedLine)) {
            suppressingPatchHelpContinuation = true;
            return true;
        }

        return false;
    };
}
