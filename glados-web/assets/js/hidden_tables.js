function hideTables(tableId) {
    const table = document.querySelectorAll('.content-card');
    table.forEach(section => {
        section.hidden = true;
    });
    document.getElementById(tableId).hidden = false;
}