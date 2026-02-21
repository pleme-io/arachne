CREATE TABLE profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    canonical_phone TEXT UNIQUE,
    name TEXT NOT NULL,
    city TEXT NOT NULL,
    state TEXT,
    age INTEGER,
    bio TEXT,
    services TEXT[] DEFAULT '{}',
    pricing JSONB DEFAULT '{}',
    body_stats JSONB DEFAULT '{}',
    scrape_count INTEGER DEFAULT 1,
    first_seen_at TIMESTAMPTZ DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ DEFAULT NOW(),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE profile_sources (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    profile_id UUID NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    site TEXT NOT NULL,
    source_url TEXT NOT NULL,
    source_id TEXT,
    raw_data JSONB,
    last_scraped_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(site, source_url)
);

CREATE TABLE photos (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    profile_id UUID NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    rustfs_path TEXT NOT NULL,
    original_url TEXT NOT NULL,
    phash BIGINT,
    position INTEGER DEFAULT 0,
    downloaded_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(profile_id, phash)
);

CREATE TABLE scrape_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site TEXT NOT NULL,
    city TEXT NOT NULL,
    started_at TIMESTAMPTZ DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    profiles_found INTEGER DEFAULT 0,
    profiles_new INTEGER DEFAULT 0,
    profiles_updated INTEGER DEFAULT 0,
    photos_downloaded INTEGER DEFAULT 0,
    errors INTEGER DEFAULT 0,
    status TEXT DEFAULT 'running'
);

CREATE TABLE cities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    state TEXT NOT NULL,
    slug TEXT NOT NULL,
    site TEXT NOT NULL,
    priority INTEGER DEFAULT 50,
    enabled BOOLEAN DEFAULT true,
    last_full_scrape TIMESTAMPTZ,
    UNIQUE(slug, site)
);

CREATE INDEX idx_profiles_city ON profiles(city);
CREATE INDEX idx_profiles_phone ON profiles(canonical_phone) WHERE canonical_phone IS NOT NULL;
CREATE INDEX idx_profiles_last_seen ON profiles(last_seen_at);
CREATE INDEX idx_profile_sources_site ON profile_sources(site, source_url);
CREATE INDEX idx_photos_phash ON photos(phash) WHERE phash IS NOT NULL;
CREATE INDEX idx_cities_priority ON cities(priority DESC, enabled) WHERE enabled = true;
