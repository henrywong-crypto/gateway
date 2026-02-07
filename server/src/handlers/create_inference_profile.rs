use axum::{
    extract::{Form, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_csrf::CsrfToken;
use inference_profiles::{create_inference_profile, create_inference_profile_record};
use models::get_models;
use myerrors::AppError;
use myhandlers::AppState;
use serde::Deserialize;
use tower_sessions::Session;

use crate::csrf::{get_authenticity_token, verify_authenticity_token};
use crate::templates::common::{common_styles, nav_menu};

#[derive(Deserialize)]
pub struct CreateInferenceProfileForm {
    pub authenticity_token: String,
    pub model_name: String,
}

pub async fn create_inference_profile_get(
    token: CsrfToken,
    session: Session,
    state: State<AppState>,
) -> Result<Response, AppError> {
    let _email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    let authenticity_token = get_authenticity_token(&token, &session).await?;

    let models = get_models(&state.db_pool).await?;

    let mut options = String::new();
    for model in models {
        options.push_str(&format!(
            r#"<option value="{}">{}</option>"#,
            model.model_name, model.model_name
        ));
    }

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            {}
        </head>
        <body>
            <div>
                <h1>Create Inference Profile</h1>
                <form action="/create-inference-profile" method="post">
                    <input type="hidden" name="authenticity_token" value="{}">
                    <label for="model_name">Model:</label><br>
                    <select id="model_name" name="model_name" required>
                        {}
                    </select><br><br>
                    <button type="submit">Create Inference Profile</button>
                </form>
                {}
            </div>
        </body>
        </html>
        "#,
        common_styles(),
        authenticity_token,
        options,
        nav_menu()
    );

    Ok((token, Html(html)).into_response())
}

pub async fn create_inference_profile_post(
    token: CsrfToken,
    session: Session,
    state: State<AppState>,
    form: Form<CreateInferenceProfileForm>,
) -> Result<Response, AppError> {
    let email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    verify_authenticity_token(&token, &session, &form.authenticity_token).await?;

    let profile_name = format!("{}-{}", email, form.model_name);

    let tags = vec![("user_email".to_string(), email.clone())];

    match create_inference_profile(&form.model_name, &profile_name, tags).await {
        Ok(arn) => {
            create_inference_profile_record(
                &state.db_pool,
                &email,
                &form.model_name,
                &arn,
                &profile_name,
            )
            .await?;

            let html = format!(
                r#"
                <!DOCTYPE html>
                <html>
                <head>
                    {}
                </head>
                <body>
                    <div>
                        <h1>Inference Profile Created</h1>
                        <p>Profile "{}" has been created successfully.</p>
                        <p>ARN: {}</p>
                        {}
                    </div>
                </body>
                </html>
                "#,
                common_styles(),
                profile_name,
                arn,
                nav_menu()
            );
            Ok((token, Html(html)).into_response())
        }
        Err(e) => {
            let error_message = if e.to_string().contains("unique constraint")
                || e.to_string().contains("already exists")
            {
                format!(
                    "An inference profile for model \"{}\" already exists.",
                    form.model_name
                )
            } else {
                format!("Failed to create inference profile: {}", e)
            };

            let html = format!(
                r#"
                <!DOCTYPE html>
                <html>
                <head>
                    {}
                </head>
                <body>
                    <div>
                        <h1>Error</h1>
                        <p style="color: red;">{}</p>
                        {}
                    </div>
                </body>
                </html>
                "#,
                common_styles(),
                error_message,
                nav_menu()
            );
            Ok((token, Html(html)).into_response())
        }
    }
}
