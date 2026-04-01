use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExchangeCodeParams {
    #[schemars(
        description = "The authorization code from the OAuth callback URL. \
                        After visiting the auth URL, copy the 'code' parameter from the redirect URL."
    )]
    pub code: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListParams {
    #[schemars(description = "Page number (default 1)")]
    pub page: Option<u32>,

    #[schemars(description = "Results per page (default 25, max 100)")]
    pub per_page: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetByIdParams {
    #[schemars(description = "The resource ID")]
    pub id: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateClientParams {
    #[schemars(description = "First name")]
    pub fname: Option<String>,

    #[schemars(description = "Last name")]
    pub lname: Option<String>,

    #[schemars(description = "Email address")]
    pub email: Option<String>,

    #[schemars(description = "Organization / company name")]
    pub organization: Option<String>,

    #[schemars(description = "Business phone number")]
    pub bus_phone: Option<String>,

    #[schemars(description = "Mobile phone number")]
    pub mob_phone: Option<String>,

    #[schemars(description = "Street address")]
    pub p_street: Option<String>,

    #[schemars(description = "City")]
    pub p_city: Option<String>,

    #[schemars(description = "Province / state")]
    pub p_province: Option<String>,

    #[schemars(description = "Country")]
    pub p_country: Option<String>,

    #[schemars(description = "Postal / zip code")]
    pub p_code: Option<String>,

    #[schemars(description = "Currency code (e.g. USD, CAD, ZAR)")]
    pub currency_code: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateClientParams {
    #[schemars(description = "Client ID to update")]
    pub id: u64,

    #[schemars(description = "First name")]
    pub fname: Option<String>,

    #[schemars(description = "Last name")]
    pub lname: Option<String>,

    #[schemars(description = "Email address")]
    pub email: Option<String>,

    #[schemars(description = "Organization / company name")]
    pub organization: Option<String>,

    #[schemars(description = "Business phone number")]
    pub bus_phone: Option<String>,

    #[schemars(description = "Mobile phone number")]
    pub mob_phone: Option<String>,

    #[schemars(description = "Street address")]
    pub p_street: Option<String>,

    #[schemars(description = "City")]
    pub p_city: Option<String>,

    #[schemars(description = "Province / state")]
    pub p_province: Option<String>,

    #[schemars(description = "Country")]
    pub p_country: Option<String>,

    #[schemars(description = "Postal / zip code")]
    pub p_code: Option<String>,

    #[schemars(description = "Currency code (e.g. USD, CAD, ZAR)")]
    pub currency_code: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateInvoiceParams {
    #[schemars(description = "Client ID for this invoice")]
    pub customerid: u64,

    #[schemars(description = "Invoice status: draft or sent (default: draft)")]
    pub status: Option<String>,

    #[schemars(
        description = "Invoice line items as JSON array. Each item: \
                        {\"name\": \"...\", \"description\": \"...\", \"qty\": 1, \"unit_cost\": {\"amount\": \"100.00\", \"code\": \"USD\"}}"
    )]
    pub lines: String,

    #[schemars(description = "Notes to display on invoice")]
    pub notes: Option<String>,

    #[schemars(description = "Invoice create date (YYYY-MM-DD, default: today)")]
    pub create_date: Option<String>,

    #[schemars(description = "Payment due date (YYYY-MM-DD)")]
    pub due_date: Option<String>,

    #[schemars(description = "Currency code (e.g. USD, CAD, ZAR)")]
    pub currency_code: Option<String>,

    #[schemars(description = "Purchase order number")]
    pub po_number: Option<String>,

    #[schemars(description = "Discount percentage (e.g. 10 for 10%)")]
    pub discount_value: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchParams {
    #[schemars(description = "Search query string")]
    pub query: String,

    #[schemars(description = "Page number (default 1)")]
    pub page: Option<u32>,

    #[schemars(description = "Results per page (default 25, max 100)")]
    pub per_page: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListExpensesParams {
    #[schemars(description = "Page number (default 1)")]
    pub page: Option<u32>,

    #[schemars(description = "Results per page (default 25, max 100)")]
    pub per_page: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateExpenseParams {
    #[schemars(description = "Expense amount")]
    pub amount: String,

    #[schemars(description = "Currency code (e.g. USD, CAD, ZAR)")]
    pub currency_code: Option<String>,

    #[schemars(description = "Expense category ID")]
    pub categoryid: Option<u64>,

    #[schemars(description = "Date of expense (YYYY-MM-DD)")]
    pub date: Option<String>,

    #[schemars(description = "Vendor name")]
    pub vendor: Option<String>,

    #[schemars(description = "Description / notes")]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListPaymentsParams {
    #[schemars(description = "Page number (default 1)")]
    pub page: Option<u32>,

    #[schemars(description = "Results per page (default 25, max 100)")]
    pub per_page: Option<u32>,
}
