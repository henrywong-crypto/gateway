CREATE TABLE IF NOT EXISTS inference_profiles (
    inference_profile_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL,
    model_id UUID NOT NULL,
    inference_profile_arn VARCHAR(512) NOT NULL,
    inference_profile_name VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT fk_inference_profiles_user_id FOREIGN KEY (user_id) REFERENCES users(user_id),
    CONSTRAINT fk_inference_profiles_model_id FOREIGN KEY (model_id) REFERENCES models(model_id),
    CONSTRAINT uq_inference_profiles_user_model UNIQUE (user_id, model_id)
);

CREATE INDEX IF NOT EXISTS idx_inference_profiles_user_id ON inference_profiles (user_id);

DROP TABLE IF EXISTS usage;

ALTER TABLE users DROP COLUMN IF EXISTS usage_tracking_enabled;
