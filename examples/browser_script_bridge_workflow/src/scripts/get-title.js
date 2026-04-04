(function(env){
  try {
    var title = document.title || '';
    return JSON.stringify({ ok: true, title: title, note: env && env.note });
  } catch (e) {
    return JSON.stringify({ success: false, message: String(e) });
  }
})