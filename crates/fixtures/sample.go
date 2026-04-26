package fixtures

import "fmt"

// Helper doubles a value.
func Helper(x int) int {
return x * 2
}

// Process sums items.
func Process(items []int) int {
// TODO: handle empty slice
total := 0
for _, item := range items {
total += Helper(item)
}
return total
}
