import asyncio
import calendar
import datetime
import pathlib
import pickle

from PIL import Image
import pyppeteer
import pytz
import flask
import google.oauth2.credentials
import google_auth_oauthlib.flow
import googleapiclient.discovery
import io
import requests

from . import secrets


class ReverseProxied(object):
    def __init__(self, app):
        self.app = app

    def __call__(self, environ, start_response):
        script_name = environ.get('HTTP_X_SCRIPT_NAME', '')
        if script_name:
            environ['SCRIPT_NAME'] = script_name
            path_info = environ['PATH_INFO']
            if path_info.startswith(script_name):
                environ['PATH_INFO'] = path_info[len(script_name):]

        scheme = environ.get('HTTP_X_SCHEME', '')
        if scheme:
            environ['wsgi.url_scheme'] = scheme
        return self.app(environ, start_response)


SCOPES = ['https://www.googleapis.com/auth/calendar', 'https://www.googleapis.com/auth/tasks.readonly']

loop = asyncio.get_event_loop()

app = flask.Flask(__name__)
app.wsgi_app = ReverseProxied(app.wsgi_app)
app.secret_key = secrets.SECRET_KEY


def weather_icon_path(darksky_icon):
    if darksky_icon == 'clear-day':
        return 'wi wi-day-sunny'
    elif darksky_icon == 'clear-night':
        return 'wi wi-night-clear'
    elif darksky_icon == 'rain':
        return 'wi wi-rain'
    elif darksky_icon == 'snow':
        return 'wi wi-snow'
    elif darksky_icon == 'sleet':
        return 'wi wi-sleet'
    elif darksky_icon == 'wind':
        return 'wi wi-wind'
    elif darksky_icon == 'fog':
        return 'wi wi-fog'
    elif darksky_icon == 'cloudy':
        return 'wi wi-cloudy'
    elif darksky_icon == 'partly-cloudy-day':
        return 'wi wi-forecast-io-partly-cloudy-day'
    elif darksky_icon == 'partly-cloudy-night':
        return 'wi wi-forecast-io-partly-cloudy-night'
    else:
        return 'wi wi-na'


def three_day_weather(latitude, longitude, location_name):
    now = datetime.datetime.now()
    resp = requests.get(f'https://api.darksky.net/forecast/{secrets.DARK_SKY_SECRET}/{latitude},{longitude}').json()
    weather_data = {
        'current_icon': weather_icon_path(resp['currently']['icon']),
        'current_temp': round(resp['currently']['temperature']),
        'current_summary': resp['currently']['summary'],
        'location_name': location_name
    }

    for i in range(0, 3):
        weather_data[f'{i}_day'] = now.strftime('%a')
        weather_data[f'{i}_icon'] = weather_icon_path(resp['daily']['data'][i]['icon'])
        weather_data[f'{i}_high'] = round(resp['daily']['data'][i]['temperatureHigh'])
        weather_data[f'{i}_low'] = round(resp['daily']['data'][i]['temperatureLow'])

        now += datetime.timedelta(days=1)

    return weather_data


@app.before_request
def before_request():
    if flask.request.url.startswith('http://'):
        url = flask.request.url.replace('http://', 'https://', 1)
        code = 301
        return flask.redirect(url, code=code)


@app.route('/')
def index():
    if 'credentials' not in flask.session:
        return flask.redirect(flask.url_for('authorize'))

    # with open('token.pickle', 'wb') as token:
    #     pickle.dump(flask.session['credentials'], token)

    # with open('token.pickle', 'rb') as token:
    #     flask.session['credentials'] = pickle.load(token)

    data = {
        'weather': three_day_weather("34.106081", "-117.710486", "Harvey Mudd")
    }

    # Get date/time data
    if hasattr(flask.g, 'tz') and flask.g.tz in pytz.all_timezones:
        now = datetime.datetime.now(tz=pytz.timezone(flask.g.tz))
    else:
        now = datetime.datetime.now()

    data['year'] = now.year
    data['month'] = now.strftime('%B')
    data['day_int'] = now.day
    data['day_str'] = now.strftime('%A')
    data['first_day_of_month'], data['days_in_month'] = calendar.monthrange(now.year, now.month)
    data['first_day_of_month'] = (data['first_day_of_month'] + 1) % 7

    # Set up Google API credentials
    credentials = google.oauth2.credentials.Credentials(**flask.session['credentials'])

    # Get Google Calendar events
    cal_service = googleapiclient.discovery.build('calendar', 'v3', credentials=credentials)
    now = datetime.datetime.utcnow().isoformat() + 'Z'
    primary_events = cal_service.events().list(calendarId='primary', timeMin=now,
                                               maxResults=10, singleEvents=True,
                                               orderBy='startTime').execute()
    # holiday_events = service.events().list(calendarId='en.usa#holiday@group.v.calendar.google.com', timeMin=now,
    #                                        maxResults=10, singleEvents=True,
    #                                        orderBy='startTime').execute()

    events = primary_events.get('items', [])  # + holiday_events.get('items', [])
    # events.sort(key=lambda x: x['start']['date'] if 'date' in x['start'] else x['start']['dateTime'])

    # Within 6 days — day of week
    # Else - month day
    data['events'] = []
    for event in events:
        start_time = None
        if 'date' in event['start']:
            start_time = datetime.datetime.strptime(event['start']['date'], '%Y-%m-%d')
        elif 'dateTime' in event['start']:
            start_time = datetime.datetime.strptime(event['start']['dateTime'], '%Y-%m-%dT%H:%M:%S%z')

        time_str = ''
        if start_time:
            if 'date' in event['end']:
                end_time = datetime.datetime.strptime(event['end']['date'], '%Y-%m-%d')
                end_time -= datetime.timedelta(days=1)

                if end_time.date() == start_time.date():
                    time_str = end_time.strftime('%b %d')
                else:
                    time_str = f"{start_time.strftime('%b %d')} - {end_time.strftime('%b %d')}"
            elif 'dateTime' in event['end']:
                end_time = datetime.datetime.strptime(event['end']['dateTime'], '%Y-%m-%dT%H:%M:%S%z')
                if end_time.date() == start_time.date():
                    time_str = f"{start_time.strftime('%b %d')} " \
                               f"{start_time.strftime('%H:%M')} - {end_time.strftime('%H:%M')}"
                else:
                    time_str = f"{start_time.strftime('%b %d %H:%M')} - {end_time.strftime('%b %d %H:%M')}"

        if time_str:
            data['events'].append({'summary': event['summary'],
                                   'time': time_str})
        else:
            data['events'].append({'summary': event['summary']})

    # for event in data['events']:
    #     print(event)
    # print()

    # Get Google Tasks
    task_service = googleapiclient.discovery.build('tasks', 'v1', credentials=credentials)

    task_list = task_service.tasklists().list(maxResults=1).execute().get('items', [])
    if task_list:
        task_results = task_service.tasks().list(tasklist=task_list[0]['id'],
                                                 showCompleted=False).execute()
        tasks = task_results.get('items', [])

        data['tasks'] = []
        for task in tasks:
            if 'due' in task:
                date = datetime.datetime.strptime(task['due'], '%Y-%m-%dT%H:%M:%S.000Z')
                due_date = date.strftime('%b %d')

                data['tasks'].append({'title': task['title'],
                                      'due_date': due_date})
            else:
                data['tasks'].append({'title': task['title']})
    else:
        data['tasks'] = []

    # for task in data['tasks']:
    #     print(task)

    return flask.render_template('index.html', **data)


async def html_to_png(content, width, height):
    # Disable signal handling to because html_to_jpg is not called in main thread, ignore certificate errors
    browser = await pyppeteer.launch(
        handleSIGINT=False,
        handleSIGTERM=False,
        handleSIGHUP=False,
        headless=True,
        ignoreHTTPSErrors=True,
        args=["--ignore-certificate-errors", "--disable-web-security"]
    )

    page = await browser.newPage()
    await page.setViewport({'width': width, 'height': height})
    await page.goto(f"data:text/html,{content}", {'waitUntil': 'networkidle2'})
    await asyncio.sleep(10)
    buffer = await page.screenshot({"quality": 100, "type": "png"})
    await browser.close()

    return buffer


@app.route("/bmp")
def bmp():
    height = flask.request.args.get('height', type=int, default=384)
    width = flask.request.args.get('width', type=int, default=640)

    # TZ should be parsable by PyTZ
    flask.g.tz = flask.request.args.get('tz', type=str, default="")

    # hacky replacement of css href tag
    base_dir = pathlib.Path(__file__).parent.absolute()
    content = index()
    if type(content) == str:
        content = content.replace('/static/',  f'file://{base_dir}/static/')
    else:
        content = content.get_data().replace('/static/',  f'file://{base_dir}/static/')

    print(content)
    image_binary = loop.run_until_complete(html_to_png(content, width, height))

    img = Image.open(io.BytesIO(image_binary))

    converted_binary = io.BytesIO()
    img.save(converted_binary, format='BMP')
    converted_binary.seek(0)

    return flask.send_file(
        converted_binary,
        mimetype='image/bmp',
        as_attachment=True,
        attachment_filename='index.bmp'
    )


@app.route("/authorize")
def authorize():
    # Taken from https://github.com/youtube/api-samples/blob/master/python/quickstart_web.py
    flow = google_auth_oauthlib.flow.Flow.from_client_secrets_file(
        'client_secrets.json',
        scopes=SCOPES,
        redirect_uri=flask.url_for("oauth2callback", _external=True))
    authorization_url, state = flow.authorization_url(
        access_type='offline',
        include_granted_scopes='true',
        prompt='consent')

    flask.session['state'] = state
    flask.session['code_verifier'] = flow.code_verifier

    return flask.redirect(authorization_url)


@app.route('/oauth2callback')
def oauth2callback():
    # Taken from https://github.com/youtube/api-samples/blob/master/python/quickstart_web.py
    state = flask.session['state']
    flow = google_auth_oauthlib.flow.Flow.from_client_secrets_file(
        'client_secrets.json',
        scopes=SCOPES,
        state=state,
        redirect_uri=flask.url_for('oauth2callback', _external=True))
    flow.code_verifier = flask.session['code_verifier']

    authorization_response = flask.request.url
    flow.fetch_token(authorization_response=authorization_response)

    credentials = flow.credentials
    flask.session['credentials'] = {
        'token': credentials.token,
        'refresh_token': credentials.refresh_token,
        'token_uri': credentials.token_uri,
        'client_id': credentials.client_id,
        'client_secret': credentials.client_secret,
        'scopes': credentials.scopes
    }

    return flask.redirect(flask.url_for('index'))


if __name__ == '__main__':
    app.run(ssl_context='adhoc')
    # app.run(host='0.0.0.0', port=5001)
