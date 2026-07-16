CREATE TABLE favorite_on_demand_categories (
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  category_id UUID NOT NULL REFERENCES on_demand_categories(id) ON DELETE CASCADE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (user_id, category_id)
);

CREATE TABLE favorite_on_demand_titles (
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  title_id UUID NOT NULL REFERENCES on_demand_titles(id) ON DELETE CASCADE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (user_id, title_id)
);

CREATE INDEX favorite_on_demand_categories_user_idx
  ON favorite_on_demand_categories(user_id, created_at);
CREATE INDEX favorite_on_demand_titles_user_idx
  ON favorite_on_demand_titles(user_id, created_at);

CREATE TABLE google_calendar_oauth_states (
  state_hash TEXT PRIMARY KEY,
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  pkce_verifier_encrypted TEXT NOT NULL,
  expires_at TIMESTAMPTZ NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX google_calendar_oauth_states_expiry_idx
  ON google_calendar_oauth_states(expires_at);

CREATE TABLE google_calendar_connections (
  user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  access_token_encrypted TEXT NOT NULL,
  refresh_token_encrypted TEXT NOT NULL,
  token_expires_at TIMESTAMPTZ NOT NULL,
  granted_scopes TEXT[] NOT NULL DEFAULT '{}',
  selected_calendar_id TEXT NULL,
  selected_calendar_name TEXT NULL,
  needs_reauthorization BOOLEAN NOT NULL DEFAULT FALSE,
  connected_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE sports_calendar_events (
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  sports_event_id TEXT NOT NULL,
  calendar_id TEXT NOT NULL,
  google_event_id TEXT NOT NULL,
  google_event_url TEXT NULL,
  event_fingerprint TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (user_id, sports_event_id, calendar_id)
);
