//! The K1 root fields the command line never uses, each with its reason (CMD-031).

pub const EXCEPTIONS: &[(&str, &str)] = &[
    (
        "Mutation.hostedDeviceInvite",
        "the Anima.ing cloud only: takes the cloud key, a secret (R-129)",
    ),
    (
        "Mutation.hostedBrowserTicket",
        "the Anima.ing cloud only: takes the cloud key, a secret (HMD-061)",
    ),
    (
        "Mutation.hostedBrowserSignIn",
        "the gateway of the pool only: sets the __Host-anima cookie for a browser (HMD-061)",
    ),
    (
        "Mutation.claimDeviceToken",
        "the desktop app's silent join through the link only: returns a token (R-129, HMD-037)",
    ),
    (
        "Mutation.claimComputerEnrolment",
        "the install script of a computer with no application only, through its link: returns \
         a daemon enrolment code (HM-T33)",
    ),
    (
        "Mutation.signInBrowser",
        "browser only: sets the __Host-anima cookie (K7)",
    ),
    (
        "Mutation.claimBuildJob",
        "builder token only: the desktop builder (BLD-022)",
    ),
    (
        "Mutation.reportBuildJob",
        "builder token only: the desktop builder (BLD-022)",
    ),
    (
        "Mutation.buildUploadUrls",
        "builder token only: the desktop builder (BLD-022)",
    ),
    (
        "Mutation.reportBuildUploaded",
        "builder token only: the desktop builder (BLD-022)",
    ),
    (
        "Mutation.offerVaultKey",
        "client work: a client adds a client by QR code; no command takes part in a key offer \
         (K-E5 Section 3, a trust decision)",
    ),
    (
        "Mutation.recordReveal",
        "client work: only the client that opens a sealed record asks for its reveal; no \
         command can open a record (K-E5 Section 3, a trust decision)",
    ),
    (
        "Query.vaultKeyOffer",
        "client work: a client adds a client by QR code; no command takes part in a key offer \
         (K-E5 Section 3, a trust decision)",
    ),
];
