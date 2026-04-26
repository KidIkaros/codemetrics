package fixtures;

import java.util.List;

public class Sample {
    /**
     * Helper doubles a value.
     */
    public static int helper(int x) {
        return x * 2;
    }

    public static int process(List<Integer> items) {
        // TODO: add null check
        int total = 0;
        for (int item : items) {
            total += helper(item);
        }
        return total;
    }
}
