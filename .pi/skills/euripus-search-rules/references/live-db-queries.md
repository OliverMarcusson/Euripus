# Live DB Queries For Euripus Search Rule Generation

Use these commands to inspect the **actual local dev database**.

## 1) Confirm containers

```bash
docker ps --format 'table {{.Names}}\t{{.Status}}\t{{.Ports}}'
```

Typical local names:

- `euripus-postgres-1`
- `euripus-server-1`
- `euripus-meilisearch-1`

If the Postgres container name differs, substitute it in the commands below.

## 2) Basic table counts

```bash
docker exec euripus-postgres-1 psql -U euripus -d euripus -c "
select count(*) as provider_profiles from provider_profiles;
select count(*) as channel_categories from channel_categories;
select count(*) as channels from channels;
select count(*) as programs from programs;
select count(*) as search_documents from search_documents;
"
```

## 3) Export the current live admin rule set

This is useful before proposing a replacement.

```bash
docker exec euripus-postgres-1 psql -U euripus -d euripus -P pager=off -t -A -F '' -c "
with grouped as (
  select
    g.id,
    g.kind,
    g.value,
    g.match_target,
    g.match_mode,
    g.priority,
    g.enabled,
    coalesce(
      (
        select json_agg(pc.country_code order by pc.country_code)
        from admin_search_provider_countries pc
        where pc.group_id = g.id
      ),
      '[]'::json
    ) as country_codes,
    coalesce(
      (
        select json_agg(p.pattern order by p.pattern)
        from admin_search_patterns p
        where p.group_id = g.id
      ),
      '[]'::json
    ) as patterns
  from admin_search_pattern_groups g
)
select jsonb_pretty(jsonb_agg(obj order by kind, value))
from (
  select
    jsonb_strip_nulls(
      jsonb_build_object(
        'kind', kind,
        'value', value,
        'matchTarget', match_target,
        'matchMode', match_mode,
        'priority', priority,
        'enabled', enabled,
        'countryCodes', case when kind = 'provider' then country_codes else null end,
        'patterns', patterns
      )
    ) as obj,
    kind,
    value
  from grouped
) s;
"
```

## 4) Country prefix counts

```bash
docker exec euripus-postgres-1 psql -U euripus -d euripus -c "
select 'channels' as src,
  count(*) filter (where name ilike 'US:%') as us_colon,
  count(*) filter (where name ilike 'US|%') as us_pipe,
  count(*) filter (where name ilike 'UK:%') as uk_colon,
  count(*) filter (where name ilike 'UK|%') as uk_pipe,
  count(*) filter (where name ilike 'SE:%') as se_colon,
  count(*) filter (where name ilike 'SE|%') as se_pipe
from channels
union all
select 'categories',
  count(*) filter (where name ilike 'US:%'),
  count(*) filter (where name ilike 'US|%'),
  count(*) filter (where name ilike 'UK:%'),
  count(*) filter (where name ilike 'UK|%'),
  count(*) filter (where name ilike 'SE:%'),
  count(*) filter (where name ilike 'SE|%')
from channel_categories;
"
```

Inspect representative examples too:

```bash
docker exec euripus-postgres-1 psql -U euripus -d euripus -c "
select name from channels
where name ilike 'US:%' or name ilike 'US|%'
order by name
limit 50;
"
```

## 5) Search for provider candidates by country prefix

Use channels + categories together.

```bash
docker exec euripus-postgres-1 psql -U euripus -d euripus -P pager=off -c "
with us_items as (
  select c.name as channel_name, cc.name as category_name
  from channels c
  left join channel_categories cc on cc.id = c.category_id
  where c.name ilike 'US:%'
     or c.name ilike 'US|%'
     or cc.name ilike 'US:%'
     or cc.name ilike 'US|%'
)
select provider, hits
from (
  values
    ('espnplus', (select count(*) from us_items where channel_name ilike '%ESPN+%' or category_name ilike '%ESPN+%' or channel_name ilike '%ESPN PLUS%' or category_name ilike '%ESPN PLUS%' or channel_name ilike '%ESPN PLAY%' or category_name ilike '%ESPN PLAY%')),
    ('flosports', (select count(*) from us_items where channel_name ilike '%FLO SPORTS%' or category_name ilike '%FLO SPORTS%' or channel_name ilike '%FLO RACING%' or category_name ilike '%FLO RACING%' or channel_name ilike '%FLO COLLEGE%' or category_name ilike '%FLO COLLEGE%' or channel_name ilike '%FLO NETWORK%' or category_name ilike '%FLO NETWORK%')),
    ('directv', (select count(*) from us_items where channel_name ilike '%DIREC TV%' or category_name ilike '%DIREC TV%' or channel_name ilike '%DIRECTV%' or category_name ilike '%DIRECTV%')),
    ('paramountplus', (select count(*) from us_items where channel_name ilike '%PARAMOUNT+%' or category_name ilike '%PARAMOUNT+%' or channel_name ilike '%PARAMOUNT PLUS%' or category_name ilike '%PARAMOUNT PLUS%')),
    ('peacock', (select count(*) from us_items where channel_name ilike '%PEACOCK%' or category_name ilike '%PEACOCK%')),
    ('max', (select count(*) from us_items where channel_name ilike '%HBO MAX%' or category_name ilike '%HBO MAX%' or channel_name ilike '%B/R MAX%' or category_name ilike '%B/R MAX%' or channel_name ilike '%MAX PPV%' or category_name ilike '%MAX PPV%' or channel_name ilike '%MAX ESPN%' or category_name ilike '%MAX ESPN%')),
    ('dazn', (select count(*) from us_items where channel_name ilike '%DAZN%' or category_name ilike '%DAZN%')),
    ('hulu', (select count(*) from us_items where channel_name ilike '%HULU%' or category_name ilike '%HULU%')),
    ('disneyplus', (select count(*) from us_items where channel_name ilike '%DISNEY+%' or category_name ilike '%DISNEY+%' or channel_name ilike '%DISNEY PLUS%' or category_name ilike '%DISNEY PLUS%')),
    ('ballysports', (select count(*) from us_items where channel_name ilike '%BALLY NETWORK%' or category_name ilike '%BALLY NETWORK%' or channel_name ilike '%BALLY SPORTS%' or category_name ilike '%BALLY SPORTS%')),
    ('fitetv', (select count(*) from us_items where channel_name ilike '%FITE TV%' or category_name ilike '%FITE TV%')),
    ('appletvplus', (select count(*) from us_items where channel_name ilike '%APPLE TV+%' or category_name ilike '%APPLE TV+%' or channel_name ilike '%APPLE TV F1%' or category_name ilike '%APPLE TV F1%'))
) as t(provider, hits)
where hits > 0
order by hits desc, provider;
"
```

Repeat the same pattern for `SE` and `UK` by changing the prefix filter and the candidate provider list.

## 6) Inspect representative rows for a candidate provider

```bash
docker exec euripus-postgres-1 psql -U euripus -d euripus -P pager=off -c "
with us_items as (
  select c.name as channel_name, cc.name as category_name
  from channels c
  left join channel_categories cc on cc.id = c.category_id
  where c.name ilike 'US:%'
     or c.name ilike 'US|%'
     or cc.name ilike 'US:%'
     or cc.name ilike 'US|%'
)
select channel_name, category_name
from us_items
where channel_name ilike '%PEACOCK%'
   or category_name ilike '%PEACOCK%'
order by channel_name, category_name
limit 20;
"
```

Use this to decide:

- whether the rule should be `channel_or_category` or `category_name`
- whether punctuation variants are needed
- whether the provider name is too ambiguous to include

## 7) Validate PPV / VIP flags against live data

```bash
docker exec euripus-postgres-1 psql -U euripus -d euripus -P pager=off -c "
with items as (
  select name from channels
  union all
  select name from channel_categories
)
select
  count(*) filter (where name ilike '%PPV%' or name ilike '%PAY PER VIEW%' or name ilike '%EVENT%') as ppv_hits,
  count(*) filter (where name ilike '%VIP%' or name ilike '%ⱽᴵᴾ%') as vip_hits
from items;
"
```

Then inspect examples:

```bash
docker exec euripus-postgres-1 psql -U euripus -d euripus -P pager=off -c "
select name from channels where name ilike '%PPV%' order by name limit 30;
select name from channels where name ilike '%VIP%' or name ilike '%ⱽᴵᴾ%' order by name limit 30;
"
```

## 8) Rule authoring reminders

- Output a **top-level JSON array** for the admin import UI
- `provider` groups must include `countryCodes`
- Use lowercase canonical `value` fields
- Include only evidence-backed providers by default
- Prefer conservative patterns over broad guesses
- Keep country groups first, then flags, then providers

## 9) Sensitive data handling

Avoid exposing raw provider credentials. If you inspect `provider_profiles`, mask or omit:

- `base_url`
- `username`
- encrypted password fields
