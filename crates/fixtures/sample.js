// Sample JavaScript module for testing
const fs = require('fs');
const path = require('path');

/**
 * A helper function.
 * @param {number} x
 * @returns {number}
 */
function helper(x) {
    return x * 2;
}

function process(items) {
    // FIXME: add validation
    let total = 0;
    for (const item of items) {
        total += helper(item);
    }
    return total;
}

module.exports = { helper, process };
