DELETE FROM on_demand_titles
WHERE name !~* '^[[:space:]]*((4K|UHD|FHD|HD)[[:space:]_:|-]+)*(EN|SE|NF|AMZ|A\+|D\+|PRMT|VP|MRVL|DSC\+|SKY|MAX|P\+|PCOK|SHWT)([[:space:]_:|-]|$)';

DELETE FROM on_demand_categories c
WHERE NOT EXISTS (
  SELECT 1
  FROM on_demand_titles t
  WHERE t.category_id = c.id
);
