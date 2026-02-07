ALTER TABLE models RENAME COLUMN model_name TO model_arn;
ALTER TABLE models ALTER COLUMN model_arn TYPE VARCHAR(512);
