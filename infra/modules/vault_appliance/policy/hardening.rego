package policy

default allow_public_ssh = false

deny contains msg if {
  some msg
  load_balancer_requires_https[msg]
}

deny contains msg if {
  some msg
  load_balancer_blocks_http[msg]
}

deny contains msg if {
  some msg
  load_balancer_must_redirect_http[msg]
}

deny contains msg if {
  some msg
  firewall_requires_load_balancer[msg]
}

deny contains msg if {
  some msg
  firewall_blocks_public_ssh[msg]
}

deny contains msg if {
  some msg
  firewall_blocks_public_sources[msg]
}

load_balancer_requires_https contains msg if {
  rc := input.resource_changes[_]
  rc.type == "digitalocean_loadbalancer"
  after := rc.change.after
  after != null
  rules := object.get(after, "forwarding_rule", [])
  unknown_rules := object.get(rc.change.after_unknown, "forwarding_rule", [])
  not https_rule_exists(rules, unknown_rules)
  msg := sprintf("load balancer %s must terminate HTTPS on port 443", [after.name])
}

load_balancer_blocks_http contains msg if {
  rc := input.resource_changes[_]
  rc.type == "digitalocean_loadbalancer"
  after := rc.change.after
  after != null
  rules := object.get(after, "forwarding_rule", [])
  unknown_rules := object.get(rc.change.after_unknown, "forwarding_rule", [])
  http_rule_exists(rules, unknown_rules)
  msg := sprintf("load balancer %s must not expose HTTP forwarding rules", [after.name])
}

load_balancer_must_redirect_http contains msg if {
  rc := input.resource_changes[_]
  rc.type == "digitalocean_loadbalancer"
  after := rc.change.after
  after != null
  not object.get(after, "redirect_http_to_https", false)
  msg := sprintf("load balancer %s must redirect HTTP traffic to HTTPS", [after.name])
}

firewall_requires_load_balancer contains msg if {
  rc := input.resource_changes[_]
  rc.type == "digitalocean_firewall"
  after := rc.change.after
  after != null
  rules := object.get(after, "inbound_rule", [])
  unknown_rules := object.get(rc.change.after_unknown, "inbound_rule", [])
  not load_balancer_rule_exists(rules, unknown_rules)
  msg := sprintf("firewall %s must allow traffic from the managed load balancer", [after.name])
}

firewall_blocks_public_ssh contains msg if {
  rc := input.resource_changes[_]
  rc.type == "digitalocean_firewall"
  after := rc.change.after
  after != null
  rules := object.get(after, "inbound_rule", [])
  rule := rules[_]
  object.get(rule, "port_range", "") == "22"
  addrs := object.get(rule, "source_addresses", [])
  "0.0.0.0/0" in addrs
  not allow_public_ssh
  msg := sprintf("firewall %s must not expose SSH to 0.0.0.0/0", [after.name])
}

firewall_blocks_public_sources contains msg if {
  rc := input.resource_changes[_]
  rc.type == "digitalocean_firewall"
  after := rc.change.after
  after != null
  name := object.get(after, "name", rc.name)
  rule := object.get(after, "inbound_rule", [])[_]
  addr := public_source_address(rule)
  msg := sprintf("firewall %s must not allow traffic from %s", [name, addr])
}

firewall_blocks_public_sources contains msg if {
  rc := input.resource_changes[_]
  rc.type == "digitalocean_firewall"
  unknown_rules := object.get(rc.change.after_unknown, "inbound_rule", [])
  addr := public_source_address(unknown_rules[_])
  after := rc.change.after
  name := rc.name
  after != null
  name := object.get(after, "name", name)
  msg := sprintf("firewall %s must not allow traffic from %s", [name, addr])
}

firewall_blocks_public_sources contains msg if {
  rc := input.resource_changes[_]
  rc.type == "digitalocean_firewall"
  rc.change.after == null
  unknown_rules := object.get(rc.change.after_unknown, "inbound_rule", [])
  addr := public_source_address(unknown_rules[_])
  msg := sprintf("firewall %s must not allow traffic from %s", [rc.name, addr])
}

https_rule_exists(rules, unknown_rules) if {
  rule := rules[_]
  lower(object.get(rule, "entry_protocol", "")) == "https"
  object.get(rule, "entry_port", 0) == 443
}

https_rule_exists(rules, unknown_rules) if {
  unknown := unknown_rules[_]
  lower(object.get(unknown, "entry_protocol", "")) == "https"
  object.get(unknown, "entry_port", 0) == 443
}

http_rule_exists(rules, unknown_rules) if {
  rule := rules[_]
  lower(object.get(rule, "entry_protocol", "")) == "http"
}

http_rule_exists(rules, unknown_rules) if {
  unknown := unknown_rules[_]
  lower(object.get(unknown, "entry_protocol", "")) == "http"
}

load_balancer_rule_exists(rules, unknown_rules) if {
  some i
  rule := rules[i]
  count(object.get(rule, "source_load_balancer_uids", [])) > 0
}

load_balancer_rule_exists(rules, unknown_rules) if {
  some i
  rule := rules[i]
  count(object.get(rule, "source_load_balancer_uids", [])) == 0
  i < count(unknown_rules)
  unknown_rule := unknown_rules[i]
  object.get(unknown_rule, "source_load_balancer_uids", false)
}

public_source_address(rule) = addr if {
  addrs := object.get(rule, "source_addresses", [])
  addr := addrs[_]
  public_source_cidr(addr)
}

public_source_cidr(addr) if {
  addr == "0.0.0.0/0"
}

public_source_cidr(addr) if {
  addr == "::/0"
}

allow_public_ssh if {
  data.allow_public_ssh
}
