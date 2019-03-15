const node = {};
node.os = nw.require('os');
node.Netmask = nw.require('netmask').Netmask;
node.net = nw.require('net');
const nwwindow = nw.Window.get();
nwwindow.setResizable(false);
nwwindow.width = 800;
nwwindow.height = 500;

let services = [];
const tryips = ['192.168.45.1', '192.168.46.2'];
let scanned = [];
let previousConnects = localStorage.getItem('connected');
if (!previousConnects) {
    previousConnects = [];
} else {
    previousConnects = JSON.parse(previousConnects);
}


function checkIP(ipAddress, timeout) {
    return new Promise(function (resolve, reject) {
        setTimeout(function () {
            reject(new Error('Timed out.'))
        }, timeout);
        fetch('http://' + ipAddress + ':5000/hostname').then(function (response) {
            return response.json();
        }).then(function (hostname) {
            hostname.ip = ipAddress;
            return hostname;
        }).then(resolve, reject);
    });
}

function buttonEnable(enabled) {
    $('#connect').prop("disabled", !enabled);
}

function updateList() {
    const table = $('<table class="table table-dark table-hover"><tbody></tbody></table>');
    const tbody = $('tbody', table);
    const promises = [];
    let ips = [];
    tryips.forEach(function (ip) {
        ips.push(ip);
    });
    services.forEach(function (service) {
        ips.push(service.ipAddress);
    });
    scanned.forEach(function (ip) {
        ips.push(ip);
    });
    previousConnects.forEach(function (ip) {
        ips.push(ip);
    });
    ips = [...new Set(ips)];
    ips.forEach(function (ip) {
        promises.push(checkIP(ip, 1000).then(function (hostname) {
            const tdname = $('<td></td>');
            tdname.text(hostname.hostname);
            const tdip = $('<td></td>');
            tdip.text(hostname.ip);
            const tr = $('<tr></tr>');
            tr.attr('data-ip', hostname.ip);
            tr.append(tdname).append(tdip);
            tbody.append(tr);
        }));
    });
    Promise.all(promises).finally(function () {
        console.log('all');
        const tr = $('#mounts table tbody tr.table-success');
        let selectedIP = null;
        if (tr) {
            selectedIP = tr.attr('data-ip');
        }
        $('#mounts table').replaceWith(table);
        $('tr', table).click(function () {
            $.each($('tr', table), function (idx, value) {
                $(value).removeClass('table-success');
            });
            $(this).addClass('table-success');
            buttonEnable(true);
        });
        if (selectedIP) {
            $('tr[data-ip="' + selectedIP + '"]').addClass('table-success');
            buttonEnable(true);
        } else if ($('tr', table).length === 1) {
            $('tr', table).addClass('table-success');
            buttonEnable(true);
        } else {
            buttonEnable(false);
        }
    });
}

function connect(ip) {
    let connected = localStorage.getItem('connected');
    if (connected) {
        connected = JSON.parse(connected);
        if (!Array.isArray(connected)) {
            connected = [];
        }
    } else {
        connected = [];
    }
    connected.push(ip);
    connected = [...new Set(connected)];
    localStorage.setItem('connected', JSON.stringify(connected));
    previousConnects = connected;
    nwwindow.setResizable(true);
    nwwindow.maximize();
    location.href = 'http://' + ip + ':5000';
}

function openPort(ip, port) {
    return new Promise(function (resolve, reject) {
        const socket = new node.net.Socket();
        socket.setTimeout(40);
        socket.on('error', function () {
            socket.destroy();
            resolve(false);
        });
        socket.on('timeout', function () {
            socket.destroy();
            resolve(false);
        });
        socket.connect(port, ip, function () {
            socket.end();
            resolve(true);
        })
    });
}

let foundIPs = [];

function scanIPs(ips, i) {
    return new Promise(function (resolve, reject) {
        if (i >= ips.length || typeof ips[i] === 'undefined') {
            resolve();
            return;
        }
        if (i % parseInt(ips.length / 100, 10) === 0) {
            $('#status').html('&nbsp;Long Scan: ' + parseInt(100.0 * i / ips.length, 10) + '%');
        } else if (i === ips.length - 1) {
            $('#status').html('&nbsp;Long Scan: 100%');
        }
        openPort(ips[i], 5000).then(function (open) {
            if (open) {
                foundIPs.push(ips[i]);
                scanned.push(ips[i]);
                updateList();
            }
            resolve(scanIPs(ips, i + 1));
        });
    });
}


function networkScan() {
    foundIPs = [];
    const network = node.os.networkInterfaces();
    const blocks = [];
    for (let iface in network) {
        network[iface].forEach(function (n) {
            if (n.family === 'IPv4' && n.address.indexOf('127') !== 0) {
                blocks.push(new node.Netmask(n.address + '/' + n.netmask));
            }
        });
    }
    const ips = [];
    let large = false;
    blocks.forEach(function (block) {
        if (block.size >= 65534) {
            large = true;
            return;
        }
        block.forEach(function (ip) {
            if (typeof(ip) !== 'undefined' && ip) {
                ips.push(ip);
                if (ips.length > 20000) {
                    $('#status').html('&nbsp;Long Scan: Error network too large to scan');
                }
            }
        });
    });
    if (ips.length === 0 && large) {
        $('#status').html('&nbsp;Long Scan: Error network too large to scan');
        return;
    }
    return scanIPs([...new Set(ips)], 0).then(function () {
        scanned = foundIPs;
        updateList();
        console.log('Found from scan:', foundIPs)
        setTimeout(networkScan, 180000);
    });
}

function main() {
    const ipregex = /^(?!\.)((^|\.)([1-9]?\d|1\d\d|2(5[0-5]|[0-4]\d))){4}$/;
    chrome.mdns.onServiceList.addListener(function (found) {
        services = found;
        updateList();
    }, {serviceType: '_sstmount._tcp.local'});
    // chrome.mdns.forceDiscovery(function() {console.log('started.');});
    setTimeout(function () {
        chrome.mdns.forceDiscovery(function () {
        });
    }, 30000);

    $('#connect').click(function () {
        const tr = $('#mounts table tbody tr.table-success');
        const ip = tr.attr('data-ip');
        connect(ip);
    });
    $('#manual_ip').keyup(function () {
        const val = $('#manual_ip').val();
        if (val.match(ipregex)) {
            $('#manual_connect').attr('disabled', false);
        } else {
            $('#manual_connect').attr('disabled', true);
        }
    });
    $('#manual_connect').click(function () {
        const ip = $('#manual_ip').val();
        if (ip.match(ipregex)) {
            //TODO: Test
            checkIP(ip, 1000).then(function () {
                connect(ip);
            }, function (err) {
                alert('Unable to connect to ' + ip);
            });
        }
    });
    updateList();
    networkScan();
}


$(main);