//

local generate_balance_flags(id_with_balances="", token="mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz") =
    if std.length(id_with_balances) == 0 then
        []
    else std.map(
        function(x) (
             local g = std.split(x, ":");
             local id = g[0];
             local amount = if std.length(g) > 1 then g[1] else "10000000000";
             "--balance-only-for-testing=" + std.join(":", [id, amount, token])
        ),
        std.split(id_with_balances, " ")
    );


local abci_34(i, user) = {
    image: "lifted/many-abci:v0.34.21",
    ports: [ (8000 + i) + ":8000" ],
    volumes: [ "./node" + i + ":/genfiles:ro" ],
    user: "" + user,
    command: [
        "many-abci",
        "--verbose", "--verbose",
        "--many", "0.0.0.0:8000",
        "--many-app", "http://ledger-" + i + ":8000",
        "--many-pem", "/genfiles/abci.pem",
        "--abci", "0.0.0.0:26658",
        "--tendermint", "http://tendermint-" + i + ":26657/"
    ],
    depends_on: [ "ledger-" + i ],
};

local abci_35(i, user) = {
    image: "lifted/many-abci:v0.35.4",
    ports: [ (8000 + i) + ":8000" ],
    volumes: [ "./node" + i + ":/genfiles:ro" ],
    user: "" + user,
    command: [
        "many-abci",
        "--verbose", "--verbose",
        "--many", "0.0.0.0:8000",
        "--many-app", "http://ledger-" + i + ":8000",
        "--many-pem", "/genfiles/abci.pem",
        "--abci", "0.0.0.0:26658",
        "--tendermint", "http://tendermint-" + i + ":26657/"
    ],
    depends_on: [ "ledger-" + i ],
};

local ledger(i, user, id_with_balances) = {
    image: "lifted/many-ledger:latest",
    user: "" + user,
    volumes: [
        "./node" + i + "/persistent-ledger:/persistent",
        "./node" + i + ":/genfiles:ro",
    ],
    command: [
        "many-ledger",
        "--verbose", "--verbose",
        "--abci",
        "--state=/genfiles/ledger_state.json5",
        "--pem=/genfiles/ledger.pem",
        "--persistent=/persistent/ledger.db",
        "--addr=0.0.0.0:8000",
    ] + generate_balance_flags(id_with_balances),
};

local tendermint_34(i, user) = {
    image: "tendermint/tendermint:v0.34.21",
    command: [
        "start",
        "--rpc.laddr", "tcp://0.0.0.0:26657",
        "--proxy_app", "tcp://abci-" + i + ":26658",
    ],
    user: "" + user,
    volumes: [
        "./node" + i + "/tendermint/:/tendermint"
    ],
    ports: [ "" + (26600 + i) + ":26600" ],
};

local tendermint_35(i, user) = {
    image: "tendermint/tendermint:v0.35.4",
    command: [
        "--log-level", "info",
        "start",
        "--rpc.laddr", "tcp://0.0.0.0:26657",
        "--proxy-app", "tcp://abci-" + i + ":26658",
    ],
    user: "" + user,
    volumes: [
        "./node" + i + "/tendermint/:/tendermint"
    ],
    ports: [ "" + (26600 + i) + ":26600" ],
};

function(nb_nodes=4, user=1000, id_with_balances="") {
    version: '3',
    services: {
        ["abci-" + i]: abci_35(i, user) for i in std.range(0, 0)
    } + {
        ["ledger-" + i]: ledger(i, user, id_with_balances) for i in std.range(0, 3)
    } + {
        ["tendermint-" + i]: tendermint_35(i, user) for i in std.range(0, 0)
    } + {
        ["abci-" + i]: abci_34(i, user) for i in std.range(1, 3)
    } + {
        ["tendermint-" + i]: tendermint_34(i, user) for i in std.range(1, 3)
    },
}
