//

local abci(i, user) = {
    image: "many/abci",
    ports: [ (8000 + i) + ":8000" ],
    volumes: [ "./node" + i + ":/genfiles:ro" ],
    user: "" + user,
    command: [
        "--many", "0.0.0.0:8000",
        "--many-app", "http://ledger-" + i + ":8000",
        "--many-pem", "/genfiles/abci.pem",
        "--abci", "0.0.0.0:26658",
        "--tendermint", "http://tendermint-" + i + ":26657/"
    ],
    depends_on: [ "ledger-" + i ],
};

local ledger(i, user) = {
    image: "many/ledger",
    user: "" + user,
    volumes: [
        "./node" + i + "/persistent-ledger:/persistent",
        "./node" + i + ":/genfiles:ro",
    ],
    command: [
        "--abci",
        "--addr", "0.0.0.0:8000",
        "--state", "/genfiles/ledger_state.json",
        "--persistent", "/persistent/ledger.db",
        "--pem", "/genfiles/ledger.pem",
    ],
};

local tendermint(i, user, tendermint_tag="v0.35.1") = {
    image: "tendermint/tendermint:" + tendermint_tag,
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

function(nb_nodes=4, user=1000) {
    version: '3',
    services: {
        ["abci-" + i]: abci(i, user) for i in std.range(0, nb_nodes - 1)
    } + {
        ["ledger-" + i]: ledger(i, user) for i in std.range(0, nb_nodes - 1)
    } + {
        ["tendermint-" + i]: tendermint(i, user) for i in std.range(0, nb_nodes - 1)
    }
}
