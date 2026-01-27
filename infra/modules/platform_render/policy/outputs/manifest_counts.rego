package platform_render.policy.outputs

deny contains msg if {
  counts := input.manifest_counts_by_module.value
  total := input.manifest_count.value
  sum_counts := sum([counts[k] | k := keys(counts)[_]])
  sum_counts != total
  msg := sprintf("manifest_count %v does not equal sum %v", [total, sum_counts])
}
