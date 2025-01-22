// Position of the vertical axis.
const X_LINE = 50;
// General-use margin size.
const MARGIN = 5;
// Position of the horizontal axis, relative to the bottom.
const Y_LINE = 35;
// Minimum distance between time tick labels.
const MIN_TICK_DIST = 50;
// Radius for rounded rectangle corners.
const RADIUS = 3;
// Height of unit boxes.
const BOX_HEIGHT = 25;
// Distance between Y tick marks on the unit graph.
const Y_TICK_DIST = BOX_HEIGHT + 2;
// Rects used for mouseover detection.
// Objects of {x, y, x2, y2, i} where `i` is the index into UNIT_DATA.
let HIT_BOXES = [];
// Index into UNIT_DATA of the last unit hovered over by mouse.
let LAST_HOVER = null;
// Key is unit index, value is {x, y, width, rmeta_x} of the box.
let UNIT_COORDS = {};
// Cache of measured text widths for SVG
let MEASURE_TEXT_CACHE = {};

// Colors from css
const getCssColor = name => getComputedStyle(document.body).getPropertyValue(name);
const TEXT_COLOR = getCssColor('--text');
const BG_COLOR = getCssColor('--background');
const CANVAS_BG = getCssColor('--canvas-background');
const AXES_COLOR = getCssColor('--canvas-axes');
const GRID_COLOR = getCssColor('--canvas-grid');
const BLOCK_COLOR = getCssColor('--canvas-block');
const CUSTOM_BUILD_COLOR = getCssColor('--canvas-custom-build');
const NOT_CUSTOM_BUILD_COLOR = getCssColor('--canvas-not-custom-build');
const DEP_LINE_COLOR = getCssColor('--canvas-dep-line');
const DEP_LINE_HIGHLIGHTED_COLOR = getCssColor('--canvas-dep-line-highlighted');
const CPU_COLOR = getCssColor('--canvas-cpu');

function render_pipeline_graph() {
  if (UNIT_DATA.length == 0) {
    return;
  }
  const min_time = document.getElementById('min-unit-time').valueAsNumber;

  const units = UNIT_DATA.filter(unit => unit.duration >= min_time);

  const graph_height = Y_TICK_DIST * units.length;
  let { canvas_width, canvas_height, graph_width, px_per_sec } = resize_graph(graph_height);
  let ctx = init_canvas('pipeline-graph', canvas_width, canvas_height);
  const container = document.getElementById('pipeline-container');
  container.style.width = canvas_width;
  container.style.height = canvas_height;

  ctx.strokeStyle = AXES_COLOR;

  // Draw the graph.
  ctx.save();
  ctx.translate(X_LINE, MARGIN);

  // Compute x,y coordinate of each block.
  // We also populate a map with the count of each unit name to disambiguate if necessary
  const unitCount = new Map();
  UNIT_COORDS = {};
  for (i=0; i<units.length; i++) {
    let unit = units[i];
    let y = i * Y_TICK_DIST + 1;
    let x = px_per_sec * unit.start;
    let rmeta_x = null;
    if (unit.rmeta_time != null) {
      rmeta_x = x + px_per_sec * unit.rmeta_time;
    }
    let width = Math.max(px_per_sec * unit.duration, 1.0);
    UNIT_COORDS[unit.i] = {x, y, width, rmeta_x};

    const count = unitCount.get(unit.name) || 0;
    unitCount.set(unit.name, count + 1);
  }

  const axis_bottom = create_axis_bottom({ canvas_height, graph_width, graph_height, px_per_sec });
  const axis_left = create_axis_left(graph_height, units.length);
  const dep_lines = create_dep_lines(units);
  const boxes = create_boxes(units, unitCount, canvas_width, px_per_sec);
  const dep_lines_hl_container = `<g id="hl-pipeline" transform="translate(${X_LINE}, ${MARGIN})"></g>`;
  const svg = document.getElementById(`pipeline-graph-svg`);
  if (svg) {
    svg.innerHTML = (
      `${axis_bottom}${axis_left}${dep_lines}${boxes}${dep_lines_hl_container}`
    );
    let g = document.getElementById('boxes');
    g.onmousemove = pipeline_mousemove;
  }
}

function create_boxes(units, unitCount, canvas_width, px_per_sec) {
  let boxes = units.map(unit => {
    const { x, y, width, rmeta_x } = UNIT_COORDS[unit.i]
    const labelName =
      (unitCount.get(unit.name) || 0) > 1
        ? `${unit.name} (v${unit.version})${unit.target}`
        : `${unit.name}${unit.target}`;
    const label = `${labelName}: ${unit.duration}s`;
    const textinfo_width = measure_text_width(label);
    const label_x = Math.min(x + 5.0, canvas_width - textinfo_width - X_LINE);
    const rmeta_rect = unit.rmeta_time ?
      `<rect
        class="rmeta"
        x="${rmeta_x}"
        y="${y}"
        rx="${RADIUS}"
        width="${px_per_sec * (unit.duration - unit.rmeta_time)}"
        height="${BOX_HEIGHT}"
      ></rect>`
      : "";
    return (
      `<g class="box ${unit.mode}" data-i="${unit.i}">
        <rect x="${x}" y="${y}" rx="${RADIUS}" width="${width}" height="${BOX_HEIGHT}"></rect>${rmeta_rect}
        <text x="${label_x}" y="${y + BOX_HEIGHT / 2}">${label}</text>
      </g>`
    )
  }).join("");
  return `<g id="boxes" transform="translate(${X_LINE}, ${MARGIN})">${boxes}</g>`
}

function measure_text_width(text) {
  if (text in MEASURE_TEXT_CACHE) {
    return MEASURE_TEXT_CACHE[text];
  }

  let div = document.createElement('DIV');
  div.innerHTML = text;
  Object.assign(div.style, {
    position: 'absolute',
    top: '-100px',
    left: '-100px',
    fontFamily: 'sans-serif',
    fontSize: '14px'
  });
  document.body.appendChild(div);
  let width = div.offsetWidth;
  document.body.removeChild(div);

  MEASURE_TEXT_CACHE[text] = width;
  return width;
}

// Create lines from the given unit to the units it unlocks.
function create_dep_lines(units) {
  const lines = units
    .filter(unit => unit.i in UNIT_COORDS)
    .map(unit => {
      const { i, unlocked_units, unlocked_rmeta_units } = unit;
      const { x: from_x, y: from_y, rmeta_x } = UNIT_COORDS[i]
      let dep_lines = unlocked_units
        .filter(unlocked => unlocked in UNIT_COORDS)
        .map(unlocked => create_one_dep_line(from_x, from_y, i, unlocked, "dep"))
        .join("");
      let rmeta_dep_lines = unlocked_rmeta_units
        .filter(unlocked => unlocked in UNIT_COORDS)
        .map(unlocked => create_one_dep_line(rmeta_x, from_y, i, unlocked, "rmeta"))
        .join("");
      return [dep_lines, rmeta_dep_lines];
    }).flat().join("");
  return `<g class="dep-lines" transform="translate(${X_LINE}, ${MARGIN})">${lines}</g>`
}

function create_one_dep_line(from_x, from_y, from_unit, to_unit, dep_type) {
  const { x: u_x, y: u_y } = UNIT_COORDS[to_unit];
  const prefix = dep_type == "rmeta" ? "rdep" : "dep";
  return (
    `<polyline
      id="${prefix}-${to_unit}"
      class="dep-line"
      data-i="${from_unit}"
      points="
        ${from_x} ${from_y + BOX_HEIGHT / 2},
        ${from_x - 5} ${from_y + BOX_HEIGHT / 2},
        ${from_x - 5} ${u_y + BOX_HEIGHT / 2},
        ${u_x}, ${u_y + BOX_HEIGHT / 2}
      ">
      </polyline>`
  )
}

function render_timing_graph() {
  if (CONCURRENCY_DATA.length == 0) {
    return;
  }
  const HEIGHT = 400;
  const AXIS_HEIGHT = HEIGHT - MARGIN - Y_LINE;
  const TOP_MARGIN = 10;
  const GRAPH_HEIGHT = AXIS_HEIGHT - TOP_MARGIN;

  const {canvas_width, graph_width, ctx} = draw_graph_axes('timing-graph', AXIS_HEIGHT);

  // Draw Y tick marks and labels.
  let max_v = 0;
  for (c of CONCURRENCY_DATA) {
    max_v = Math.max(max_v, c.active, c.waiting, c.inactive);
  }
  const px_per_v = GRAPH_HEIGHT / max_v;
  const {step, tick_dist, num_ticks} = split_ticks(max_v, px_per_v, GRAPH_HEIGHT);
  ctx.textAlign = 'end';
  for (n=0; n<num_ticks; n++) {
    let y = HEIGHT - Y_LINE - ((n + 1) * tick_dist);
    ctx.beginPath();
    ctx.moveTo(X_LINE, y);
    ctx.lineTo(X_LINE-5, y);
    ctx.stroke();
    ctx.fillText((n+1) * step, X_LINE-10, y+5);
  }

  // Label the Y axis.
  let label_y = (HEIGHT - Y_LINE) / 2;
  ctx.save();
  ctx.translate(15, label_y);
  ctx.rotate(3*Math.PI/2);
  ctx.textAlign = 'center';
  ctx.fillText('# Units', 0, 0);
  ctx.restore();

  // Draw the graph.
  ctx.save();
  ctx.translate(X_LINE, MARGIN);

  function coord(t, v) {
    return {
      x: graph_width * (t/DURATION),
      y: TOP_MARGIN + GRAPH_HEIGHT * (1.0 - (v / max_v))
    };
  }

  const cpuFillStyle = CPU_COLOR;
  if (CPU_USAGE.length > 1) {
    ctx.beginPath();
    ctx.fillStyle = cpuFillStyle;
    let bottomLeft = coord(CPU_USAGE[0][0], 0);
    ctx.moveTo(bottomLeft.x, bottomLeft.y);
    for (let i=0; i < CPU_USAGE.length; i++) {
      let [time, usage] = CPU_USAGE[i];
      let {x, y} = coord(time, usage / 100.0 * max_v);
      ctx.lineTo(x, y);
    }
    let bottomRight = coord(CPU_USAGE[CPU_USAGE.length - 1][0], 0);
    ctx.lineTo(bottomRight.x, bottomRight.y);
    ctx.fill();
  }

  function draw_line(style, key) {
    let first = CONCURRENCY_DATA[0];
    let last = coord(first.t, key(first));
    ctx.strokeStyle = style;
    ctx.beginPath();
    ctx.moveTo(last.x, last.y);
    for (let i=1; i<CONCURRENCY_DATA.length; i++) {
      let c = CONCURRENCY_DATA[i];
      let {x, y} = coord(c.t, key(c));
      ctx.lineTo(x, last.y);
      ctx.lineTo(x, y);
      last = {x, y};
    }
    ctx.stroke();
  }

  draw_line('blue', function(c) {return c.inactive;});
  draw_line('red', function(c) {return c.waiting;});
  draw_line('green', function(c) {return c.active;});

  // Draw a legend.
  ctx.restore();
  ctx.save();
  ctx.translate(canvas_width-200, MARGIN);
  // background
  ctx.fillStyle = BG_COLOR;
  ctx.strokeStyle = TEXT_COLOR;
  ctx.lineWidth = 1;
  ctx.textBaseline = 'middle'
  ctx.textAlign = 'start';
  ctx.beginPath();
  ctx.rect(0, 0, 150, 82);
  ctx.stroke();
  ctx.fill();

  ctx.fillStyle = TEXT_COLOR;
  ctx.beginPath();
  ctx.lineWidth = 2;
  ctx.strokeStyle = 'red';
  ctx.moveTo(5, 10);
  ctx.lineTo(50, 10);
  ctx.stroke();
  ctx.fillText('Waiting', 54, 11);

  ctx.beginPath();
  ctx.strokeStyle = 'blue';
  ctx.moveTo(5, 30);
  ctx.lineTo(50, 30);
  ctx.stroke();
  ctx.fillText('Inactive', 54, 31);

  ctx.beginPath();
  ctx.strokeStyle = 'green';
  ctx.moveTo(5, 50);
  ctx.lineTo(50, 50);
  ctx.stroke();
  ctx.fillText('Active', 54, 51);

  ctx.beginPath();
  ctx.fillStyle = cpuFillStyle
  ctx.fillRect(15, 60, 30, 15);
  ctx.fill();
  ctx.fillStyle = TEXT_COLOR;
  ctx.fillText('CPU Usage', 54, 71);

  ctx.restore();
}

function setup_canvas(id, width, height) {
  let g = document.getElementById(id);
  let dpr = window.devicePixelRatio || 1;
  g.width = width * dpr;
  g.height = height * dpr;
  g.style.width = width;
  g.style.height = height;
  let ctx = g.getContext('2d');
  ctx.scale(dpr, dpr);
  return ctx;
}

function resize_graph(graph_height) {
  const scale = document.getElementById('scale').valueAsNumber;
  // Cap the size of the graph. It is hard to view if it is too large, and
  // browsers may not render a large graph because it takes too much memory.
  // 4096 is still ridiculously large, and probably won't render on mobile
  // browsers, but should be ok for many desktop environments.
  const graph_width = Math.min(scale * DURATION, 4096);
  const px_per_sec = graph_width / DURATION;
  const canvas_width = Math.max(graph_width + X_LINE + 30, X_LINE + 250);
  const canvas_height = graph_height + MARGIN + Y_LINE;
  return { canvas_width, canvas_height, graph_width, graph_height, px_per_sec };
}

function draw_graph_axes(id, graph_height) {
  let { canvas_width, canvas_height, graph_width, px_per_sec } = resize_graph(graph_height);
  let ctx = setup_canvas(id, canvas_width, canvas_height);
  ctx.fillStyle = CANVAS_BG;
  ctx.fillRect(0, 0, canvas_width, canvas_height);

  ctx.lineWidth = 2;
  ctx.font = '16px sans-serif';
  ctx.textAlign = 'center';
  ctx.strokeStyle = AXES_COLOR;

  // Draw main axes.
  ctx.beginPath();
  ctx.moveTo(X_LINE, MARGIN);
  ctx.lineTo(X_LINE, graph_height + MARGIN);
  ctx.lineTo(X_LINE+graph_width+20, graph_height + MARGIN);
  ctx.stroke();

  // Draw X tick marks.
  const {step, tick_dist, num_ticks} = split_ticks(DURATION, px_per_sec, graph_width);
  ctx.fillStyle = AXES_COLOR;
  for (let n=0; n<num_ticks; n++) {
    const x = X_LINE + ((n + 1) * tick_dist);
    ctx.beginPath();
    ctx.moveTo(x, canvas_height-Y_LINE);
    ctx.lineTo(x, canvas_height-Y_LINE+5);
    ctx.stroke();

    ctx.fillText(`${(n+1) * step}s`, x, canvas_height - Y_LINE + 20);
  }

  // Draw vertical lines.
  ctx.strokeStyle = GRID_COLOR;
  ctx.setLineDash([2, 4]);
  for (n=0; n<num_ticks; n++) {
    const x = X_LINE + ((n + 1) * tick_dist);
    ctx.beginPath();
    ctx.moveTo(x, MARGIN);
    ctx.lineTo(x, MARGIN+graph_height);
    ctx.stroke();
  }
  ctx.strokeStyle = TEXT_COLOR;
  ctx.setLineDash([]);
  return { canvas_width, canvas_height, graph_width, graph_height, ctx, px_per_sec };
}

function create_axis_bottom({ canvas_height, graph_width, graph_height, px_per_sec }) {
  const { step, tick_dist, num_ticks } = split_ticks(DURATION, px_per_sec, graph_width);
  const grid_height = canvas_height - Y_LINE - MARGIN;
  const ticks = Array(num_ticks).fill(0).map((_, idx) => {
    const i = idx + 1;
    const time = i * step;
    return (
      `<g class="tick" transform="translate(${i * tick_dist}, ${grid_height})">
         <line y2="5"></line>
         <line class="grid" y1="-1" y2="-${grid_height}"></line>
         <text y="1em">${time}s</text>
       </g>`
    )
  }).join("");

  const height = graph_height;
  const width = graph_width + 20;
  return (
    `<g class="axis" transform="translate(${X_LINE}, ${MARGIN})" text-anchor="middle">
       <line class="domain" x2="${width}" y1="${height}" y2="${height}"></line>
       ${ticks}
     </g>`
  );
}

function create_axis_left(graph_height, ticks_num) {
  const text_offset = -Y_TICK_DIST / 2;
  const ticks = Array(ticks_num).fill(0).map((_, idx) => {
    const i = idx + 1;
    let mark = (i == ticks_num) ? "" :
      `<line stroke="currentColor" stroke-width="2" x2="-5"></line>`;
    return (
      `<g class="tick" transform="translate(0, ${i * Y_TICK_DIST})">
        ${mark}<text x="-5" y="${text_offset}">${i}</text>
      </g>`
    )
  }).join("");

  const height = graph_height + 1;
  return (
    `<g class="axis" transform="translate(${X_LINE}, ${MARGIN})" text-anchor="end">
      <line class="domain" y2="${height}"></line>
      ${ticks}
    </g>`
  )
}

// Determine the spacing and number of ticks along an axis.
function split_ticks(max_value, px_per_v, max_px) {
  const max_ticks = Math.floor(max_px / MIN_TICK_DIST);
  if (max_ticks <= 1) {
    // Graph is too small for even 1 tick.
    return {step: max_value, tick_dist: max_px, num_ticks: 1};
  }
  let step;
  if (max_value <= max_ticks) {
    step = 1;
  } else if (max_value <= max_ticks * 2) {
    step = 2;
  } else if (max_value <= max_ticks * 4) {
    step = 4;
  } else if (max_value <= max_ticks * 5) {
    step = 5;
  } else {
    step = 10;
    let count = 0;
    while (true) {
      if (count > 100) {
        throw Error("tick loop too long");
      }
      count += 1;
      if (max_value <= max_ticks * step) {
        break;
      }
      step += 10;
    }
  }
  const tick_dist = px_per_v * step;
  const num_ticks = Math.floor(max_value / step);
  return {step, tick_dist, num_ticks};
}

function codegen_time(unit) {
  if (unit.rmeta_time == null) {
    return null;
  }
  let ctime = unit.duration - unit.rmeta_time;
  return [unit.rmeta_time, ctime];
}

function roundedRect(ctx, x, y, width, height, r) {
  r = Math.min(r, width, height);
  ctx.beginPath();
  ctx.moveTo(x+r, y);
  ctx.lineTo(x+width-r, y);
  ctx.arc(x+width-r, y+r, r, 3*Math.PI/2, 0);
  ctx.lineTo(x+width, y+height-r);
  ctx.arc(x+width-r, y+height-r, r, 0, Math.PI/2);
  ctx.lineTo(x+r, y+height);
  ctx.arc(x+r, y+height-r, r, Math.PI/2, Math.PI);
  ctx.lineTo(x, y-r);
  ctx.arc(x+r, y+r, r, Math.PI, 3*Math.PI/2);
  ctx.closePath();
}

function pipeline_mouse_hit(event) {
  const target = event.target;
  if (target.tagName == 'rect') {
    return target.parentNode.dataset.i;
  }
}

function pipeline_mousemove(event) {
  // Highlight dependency lines on mouse hover.
  let i = pipeline_mouse_hit(event);
  if (i && i != LAST_HOVER) {
		let deps =
    document.querySelectorAll(`.dep-line[data-i="${LAST_HOVER}"],#dep-${LAST_HOVER},#rdep-${LAST_HOVER}`);
    for (let el of deps) {
      el.classList.remove('hl');
    }

    LAST_HOVER = i;
    deps = document.querySelectorAll(`.dep-line[data-i="${LAST_HOVER}"],#dep-${LAST_HOVER},#rdep-${LAST_HOVER}`);
    let ids = [];
    for (let el of deps) {
      el.classList.add('hl');
      ids.push(el.id);
    }

    let hl = document.getElementById('hl-pipeline');
    if (hl) {
      hl.innerHTML = ids.map(id => `<use xlink:href="#${id}"/>`).join('');
    }
  }
}

render_pipeline_graph();
render_timing_graph();

// Set up and handle controls.
{
  const range = document.getElementById('min-unit-time');
  const time_output = document.getElementById('min-unit-time-output');
  time_output.innerHTML = `${range.value}s`;
  range.oninput = event => {
    time_output.innerHTML = `${range.value}s`;
    render_pipeline_graph();
  };

  const scale = document.getElementById('scale');
  const scale_output = document.getElementById('scale-output');
  scale_output.innerHTML = `${scale.value}`;
  scale.oninput = event => {
    scale_output.innerHTML = `${scale.value}`;
    render_pipeline_graph();
    render_timing_graph();
  };
}
