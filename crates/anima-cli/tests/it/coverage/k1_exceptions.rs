//! The K1 root fields the command line never uses, each with its reason (CMD-031).

pub const EXCEPTIONS: &[(&str, &str)] = &[
    (
        "Mutation.login",
        "password sign-in belongs to the app; the command uses browser approval (CMD-010)",
    ),
    (
        "Mutation.commandLineSignIn",
        "approval details are for the signed-in app or browser, never an agent (SGN-NEW-approve)",
    ),
    (
        "Mutation.approveCommandLineSignIn",
        "a person approves in the app or browser, never through the CLI or MCP (SGN-NEW-approve)",
    ),
    (
        "Mutation.managedCodePreviewHandoff",
        "Code app browser/native handoff only: returns a one-use secret transferred directly to the isolated preview origin (contracts/managed-code.md)",
    ),
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
        "Mutation.hostedCommandLineClaim",
        "the account site only: a cloud-authorized browser approval (SGN-NEW-hosted)",
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
