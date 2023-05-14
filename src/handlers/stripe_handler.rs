use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

use crate::{
    data::models::{Pool, StripeCustomer},
    operators::stripe_customer_operator::{
        cancel_stripe_subscription_operation, create_stripe_checkout_session_operation,
        get_stripe_customer_query, get_user_plan_query, handle_webhook_query,
    },
};

use super::auth_handler::LoggedUser;

#[derive(Debug, Deserialize, Serialize)]
pub struct StripeCheckoutSessionResponseDTO {
    checkout_session_url: String,
}

pub async fn create_stripe_checkout_session(
    plan_id: web::Path<String>,
    user: Option<LoggedUser>,
    pool: web::Data<Pool>,
) -> Result<HttpResponse, actix_web::Error> {
    let app_url: String =
        std::env::var("APP_URL").unwrap_or_else(|_| "http://localhost:3000".into());
    let user_one = user.clone();
    let user_two = user.clone();

    let stripe_customer: Option<StripeCustomer> = match user_one {
        Some(user) => Some(
            web::block(move || get_stripe_customer_query(user.email, &pool))
                .await?
                .map_err(actix_web::error::ErrorInternalServerError)?,
        ),
        None => None,
    };
    let success_url = match user_two {
        Some(_user) => format!("{}/debate", app_url),
        None => format!("{}/payment/success", app_url),
    };

    let checkout_session_url = create_stripe_checkout_session_operation(
        stripe_customer,
        plan_id.into_inner(),
        success_url,
    )
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(StripeCheckoutSessionResponseDTO {
        checkout_session_url,
    }))
}

pub async fn cancel_subscription(
    user: LoggedUser,
    pool: web::Data<Pool>,
) -> Result<HttpResponse, actix_web::Error> {
    let plan = web::block(move || get_user_plan_query(user.email, &pool)).await?;

    if let Err(e) = plan {
        return Ok(HttpResponse::BadRequest().json(e));
    }

   match cancel_stripe_subscription_operation(plan.unwrap().stripe_subscription_id).await {
        Ok(_) => Ok(HttpResponse::Ok().finish()),
        Err(err) => Ok(HttpResponse::BadRequest().json(err))
   }
}

pub async fn get_subscription(
    user: LoggedUser,
    pool: web::Data<Pool>,
) -> Result<HttpResponse, actix_web::Error> {
    let plan = web::block(move || get_user_plan_query(user.email, &pool)).await?;

    match plan {
        Ok(plan) => Ok(HttpResponse::Ok().json(plan)),
        Err(err) => Ok(HttpResponse::BadRequest().json(err)),
    }
}

pub async fn stripe_webhook(
    req: HttpRequest,
    payload: web::Bytes,
    pool: web::Data<Pool>,
) -> Result<HttpResponse, actix_web::Error> {
    let stripe_signature = match get_header_value(&req, "Stripe-Signature") {
        None => return Ok(HttpResponse::BadRequest().finish()),
        Some(stripe_signature) => stripe_signature,
    };

    let _ = web::block(move || handle_webhook_query(&stripe_signature, payload, &pool))
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().finish())
}

fn get_header_value<'b>(req: &'b HttpRequest, key: &'b str) -> Option<String> {
    let header_val = req.headers().get(key)?.to_str().ok();

    header_val.map(|val| val.to_string())
}