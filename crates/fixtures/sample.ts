// Sample TypeScript module for testing
import * as fs from 'fs';

/**
 * A helper function.
 */
function helper(x: number): number {
    return x * 2;
}

function process(items: number[]): number {
    // HACK: naive loop
    let total = 0;
    for (const item of items) {
        total += helper(item);
    }
    return total;
}

export { helper, process };
