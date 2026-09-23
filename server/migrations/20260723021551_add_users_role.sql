ALTER TABLE users
ADD COLUMN role TEXT NOT NULL DEFAULT 'user';

UPDATE users
SET role = 'user'
WHERE role IS NULL
   OR TRIM(role) = '';
