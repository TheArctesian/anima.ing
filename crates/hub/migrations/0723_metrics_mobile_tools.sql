-- THR-172, K8 Section 10: replaced with each complete inventory, including NULL on downgrade.
ALTER TABLE metrics.inventories ADD COLUMN mobile_tools jsonb;
