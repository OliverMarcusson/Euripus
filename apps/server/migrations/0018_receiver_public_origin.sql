ALTER TABLE receiver_devices
ADD COLUMN IF NOT EXISTS last_public_origin TEXT;
